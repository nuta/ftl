use core::marker::PhantomData;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::quota::Resource;
use ftl_types::quota::UNLIMITED;
use ftl_types::thread::SyscallRegs;
use ftl_types::time::Duration;
use ftl_types::time::MonoTime;
use ftl_utils::spinlock::SpinLock;

use crate::handle::Handle;
use crate::handle::Handleable;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

const MAX_DEPTH: usize = 4;

pub trait Gauge {
    fn entry(quota: &Quota) -> &SpinLock<GaugeEntry>;
}

pub struct MemoryQuota;

impl Gauge for MemoryQuota {
    fn entry(quota: &Quota) -> &SpinLock<GaugeEntry> {
        &quota.memory
    }
}

pub trait Periodic {
    const PERIOD: Duration;

    fn entry(quota: &Quota) -> &SpinLock<PeriodicEntry>;
}

pub struct CpuTimeQuota;

impl Periodic for CpuTimeQuota {
    const PERIOD: Duration = Duration::from_millis(100);

    fn entry(quota: &Quota) -> &SpinLock<PeriodicEntry> {
        &quota.cpu_time
    }
}

pub struct GaugeEntry {
    usage: usize,
    limit: usize,
}

impl GaugeEntry {
    const fn new(limit: usize) -> Self {
        Self { usage: 0, limit }
    }

    fn try_charge(&mut self, amount: usize) -> bool {
        let Some(new_usage) = self.usage.checked_add(amount) else {
            return false;
        };

        if new_usage > self.limit {
            return false;
        }

        self.usage = new_usage;
        true
    }

    fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }

    fn uncharge(&mut self, amount: usize) {
        debug_assert!(self.usage >= amount, "quota usage underflow");
        self.usage -= amount;
    }
}

pub struct PeriodicEntry {
    period: u64,
    usage: u64,
    limit: usize,
}

impl PeriodicEntry {
    const fn new(limit: usize) -> Self {
        Self {
            period: 0,
            usage: 0,
            limit,
        }
    }

    fn is_unlimited(&self) -> bool {
        self.limit == UNLIMITED
    }

    fn charge(&mut self, current_period: u64, elapsed: Duration) {
        if self.is_unlimited() {
            return;
        }

        // Reset the usage to 0 if the period has changed.
        if current_period > self.period {
            self.period = current_period;
            self.usage = 0;
        }

        self.usage = self.usage.saturating_add(elapsed.as_nanos());
    }

    fn is_exhausted(&self, current_period: u64) -> bool {
        if self.is_unlimited() {
            return false;
        }

        let usage = if self.period < current_period {
            0
        } else {
            self.usage
        };

        usage >= self.limit as u64
    }

    fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }
}

pub struct Quota {
    parent: Option<SharedRef<Quota>>,
    depth: usize,
    memory: SpinLock<GaugeEntry>,
    cpu_time: SpinLock<PeriodicEntry>,
}

impl Quota {
    pub fn new_root() -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self::do_new(None, 0, UNLIMITED))
    }

    pub fn new(parent: SharedRef<Self>) -> Result<SharedRef<Self>, ErrorCode> {
        if parent.depth >= MAX_DEPTH {
            return Err(ErrorCode::TooDeep);
        }

        let depth = parent.depth + 1;
        SharedRef::new(Self::do_new(Some(parent), depth, 0))
    }

    fn do_new(parent: Option<SharedRef<Self>>, depth: usize, limit: usize) -> Self {
        Self {
            parent,
            depth,
            memory: SpinLock::new(GaugeEntry::new(limit)),
            cpu_time: SpinLock::new(PeriodicEntry::new(limit)),
        }
    }

    fn ancestors(&self) -> impl Iterator<Item = &Quota> {
        core::iter::successors(Some(self), |quota| quota.parent.as_deref())
    }

    fn try_charge<R: Gauge>(&self, amount: usize) -> Result<(), ErrorCode> {
        // Traverse the quota tree and charge the resource from the each level.
        for quota in self.ancestors() {
            if !R::entry(quota).lock().try_charge(amount) {
                // Failed to charge. Revert the changes we've made so far.
                for charged in self.ancestors() {
                    if core::ptr::eq(charged, quota) {
                        // Reached the level we've failed to charge.
                        break;
                    }

                    R::entry(charged).lock().uncharge(amount);
                }

                return Err(ErrorCode::QuotaExceeded);
            }
        }

        Ok(())
    }

    fn uncharge<R: Gauge>(&self, amount: usize) {
        // Traverse the quota tree and uncharge the resource from the each level.
        for quota in self.ancestors() {
            R::entry(quota).lock().uncharge(amount);
        }
    }

    pub fn reserve<R: Gauge>(&self, amount: usize) -> Result<Reservation<'_, R>, ErrorCode> {
        self.try_charge::<R>(amount)?;
        Ok(Reservation {
            quota: self,
            amount,
            _resource: PhantomData,
        })
    }

    pub fn charge_periodic<R: Periodic>(&self, now: MonoTime, elapsed: Duration) {
        let current_period = now.as_nanos() / R::PERIOD.as_nanos();

        // Traverse the quota tree and charge the resource from the each level.
        for quota in self.ancestors() {
            R::entry(quota).lock().charge(current_period, elapsed);
        }
    }

    pub fn is_exhausted<R: Periodic>(&self, now: MonoTime) -> bool {
        let current_period = now.as_nanos() / R::PERIOD.as_nanos();

        // Traverse the quota tree and check if any level has used up the resource.
        self.ancestors()
            .any(|quota| R::entry(quota).lock().is_exhausted(current_period))
    }

    /// Updates the limit for a resource.
    pub fn limit(&self, resource: Resource, limit: usize) {
        match resource {
            Resource::Memory => self.memory.lock().set_limit(limit),
            Resource::CpuTime => self.cpu_time.lock().set_limit(limit),
        }
    }
}

impl Handleable for Quota {}

pub struct Charge<R: Gauge> {
    quota: SharedRef<Quota>,
    amount: usize,
    _resource: PhantomData<R>,
}

impl<R: Gauge> Charge<R> {
    pub fn new(quota: SharedRef<Quota>) -> Self {
        Self {
            quota,
            amount: 0,
            _resource: PhantomData,
        }
    }

    pub fn commit(&mut self, mut reservation: Reservation<'_, R>) {
        assert!(core::ptr::eq(&*self.quota, reservation.quota));
        self.amount += reservation.amount;
        reservation.amount = 0;
    }

    /// Reserves the amount of resource for `f`, runs `f`, and commits if
    /// successful.
    pub fn charge_with<T>(
        &mut self,
        amount: usize,
        f: impl FnOnce() -> Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        let mut reservation = self.quota.reserve::<R>(amount)?;
        let value = f()?;
        self.amount += reservation.amount;
        reservation.amount = 0;
        Ok(value)
    }
}

impl<R: Gauge> Drop for Charge<R> {
    fn drop(&mut self) {
        self.quota.uncharge::<R>(self.amount);
    }
}

pub struct Reservation<'a, R: Gauge> {
    quota: &'a Quota,
    amount: usize,
    _resource: PhantomData<R>,
}

impl<R: Gauge> Drop for Reservation<'_, R> {
    fn drop(&mut self) {
        self.quota.uncharge::<R>(self.amount);
    }
}

pub fn sys_quota_create(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let parent_id = HandleId::new(ctx.a0);

    let hspace = current.hspace();
    let parent = hspace.get::<Quota>(parent_id, HandleRight::CHARGE)?;
    let quota = Quota::new(parent)?;
    let handle = Handle::new(quota, HandleRight::CHARGE | HandleRight::WRITE);
    let id = hspace.insert(handle)?;
    Ok(SyscallOutput::Done(id.as_usize()))
}

pub fn sys_quota_limit(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let id = HandleId::new(ctx.a0);
    let resource = Resource::from_usize(ctx.a1).ok_or(ErrorCode::InvalidArg)?;
    let limit = ctx.a2;

    let quota = current.hspace().get::<Quota>(id, HandleRight::WRITE)?;
    quota.limit(resource, limit);
    Ok(SyscallOutput::Done(0))
}
