use core::marker::PhantomData;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::quota::Resource;
use ftl_types::quota::UNLIMITED;
use ftl_types::thread::SyscallRegs;

use crate::handle::Handle;
use crate::handle::Handleable;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

const MAX_DEPTH: usize = 4;

pub trait ResourceKind {
    const RESOURCE: Resource;
}

pub struct MemoryQuota;

impl ResourceKind for MemoryQuota {
    const RESOURCE: Resource = Resource::Memory;
}

struct Entry {
    usage: AtomicUsize,
    limit: AtomicUsize,
}

impl Entry {
    const fn new(limit: usize) -> Self {
        Self {
            usage: AtomicUsize::new(0),
            limit: AtomicUsize::new(limit),
        }
    }

    fn try_charge(&self, amount: usize) -> bool {
        let limit = self.limit.load(Ordering::Relaxed);
        let result = self
            .usage
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |usage| {
                let new_usage = usage.checked_add(amount)?;
                if new_usage > limit {
                    return None;
                }

                Some(new_usage)
            });

        result.is_ok()
    }

    fn limit(&self, limit: usize) {
        self.limit.store(limit, Ordering::Relaxed);
    }

    fn uncharge(&self, amount: usize) {
        let prev = self.usage.fetch_sub(amount, Ordering::Relaxed);
        debug_assert!(prev >= amount, "quota usage underflow");
    }
}

pub struct Quota {
    parent: Option<SharedRef<Quota>>,
    depth: usize,
    quotas: [Entry; Resource::COUNT],
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
            quotas: core::array::from_fn(|_| Entry::new(limit)),
        }
    }

    fn try_charge(&self, resource: Resource, amount: usize) -> Result<(), ErrorCode> {
        // Traverse the quota tree and charge the resource from the each level.
        let mut current = Some(self);
        while let Some(quota) = current {
            if quota.quotas[resource as usize].try_charge(amount) {
                // Successfully charged. Move to the parent.
                current = quota.parent.as_deref();
                continue;
            }

            // Failed to charge. Revert the changes we've made so far.
            let mut current = Some(self);
            while let Some(charged) = current {
                if core::ptr::eq(charged, quota) {
                    // Reached the level we've failed to charge.
                    break;
                }

                charged.quotas[resource as usize].uncharge(amount);
                current = charged.parent.as_deref();
            }

            return Err(ErrorCode::QuotaExceeded);
        }

        Ok(())
    }

    fn uncharge(&self, resource: Resource, amount: usize) {
        // Traverse the quota tree and uncharge the resource from the each level.
        let mut current = Some(self);
        while let Some(quota) = current {
            quota.quotas[resource as usize].uncharge(amount);
            current = quota.parent.as_deref();
        }
    }

    pub fn reserve<R: ResourceKind>(&self, amount: usize) -> Result<Reservation<'_, R>, ErrorCode> {
        self.try_charge(R::RESOURCE, amount)?;
        Ok(Reservation {
            quota: self,
            amount,
            _resource: PhantomData,
        })
    }

    /// Updates the limit for a resource.
    pub fn limit(&self, resource: Resource, limit: usize) {
        self.quotas[resource as usize].limit(limit);
    }
}

impl Handleable for Quota {}

pub struct Charge<R: ResourceKind> {
    quota: SharedRef<Quota>,
    amount: usize,
    _resource: PhantomData<R>,
}

impl<R: ResourceKind> Charge<R> {
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

impl<R: ResourceKind> Drop for Charge<R> {
    fn drop(&mut self) {
        self.quota.uncharge(R::RESOURCE, self.amount);
    }
}

pub struct Reservation<'a, R: ResourceKind> {
    quota: &'a Quota,
    amount: usize,
    _resource: PhantomData<R>,
}

impl<R: ResourceKind> Drop for Reservation<'_, R> {
    fn drop(&mut self) {
        self.quota.uncharge(R::RESOURCE, self.amount);
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
