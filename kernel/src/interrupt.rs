use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::EventHeader;
use ftl_types::sink::EventType;

use crate::arch;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::Isolation;
use crate::isolation::UserSlice;
use crate::process::HandleTable;
use crate::shared_ref::SharedRef;
use crate::sink::Notifier;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Thread;

struct Mutable {
    notifier: Option<Notifier>,
    pending: bool,
}

pub struct Interrupt {
    irq: u8,
    mutable: SpinLock<Mutable>,
}

impl Interrupt {
    pub fn new(irq: u8) -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            irq,
            mutable: SpinLock::new(Mutable {
                notifier: None,
                pending: false,
            }),
        })
    }

    pub fn notify(&self) {
        let mut mutable = self.mutable.lock();
        mutable.pending = true;
        if let Some(ref notifier) = mutable.notifier {
            notifier.notify();
        }
    }
}

impl Handleable for Interrupt {
    fn set_notifier(&self, notifier: Notifier) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.notifier.is_some() {
            return Err(ErrorCode::AlreadyExists);
        }

        mutable.notifier = Some(notifier);
        Ok(())
    }

    fn remove_notifier(&self) {
        let mut mutable = self.mutable.lock();
        debug_assert!(mutable.notifier.is_some());
        mutable.notifier = None;
    }

    fn poll(
        &self,
        handle_id: HandleId,
        _handle_table: &mut HandleTable,
        isolation: &SharedRef<dyn Isolation>,
        buf: &UserSlice,
    ) -> Result<bool, ErrorCode> {
        let mut mutable = self.mutable.lock();
        if !mutable.pending {
            return Ok(false);
        }

        mutable.pending = false;

        let header = EventHeader {
            ty: EventType::IRQ,
            id: handle_id,
            reserved: 0,
        };
        crate::isolation::write(isolation, &buf, 0, header)?;
        crate::isolation::write(isolation, &buf, size_of::<EventHeader>(), self.irq)?;
        Ok(true)
    }
}

static INTERRUPTS: SpinLock<[Option<SharedRef<Interrupt>>; 256]> =
    SpinLock::new([const { None }; 256]);

pub fn notify_irq(irq: u8) {
    let interrupts = INTERRUPTS.lock();
    if let Some(ref interrupt) = interrupts[irq as usize] {
        interrupt.notify();
    }
}

pub fn sys_interrupt_acquire(
    current: &SharedRef<Thread>,
    a0: usize,
) -> Result<SyscallResult, ErrorCode> {
    let irq = a0 as u8;

    let mut interrupts = INTERRUPTS.lock();
    if interrupts[irq as usize].is_some() {
        return Err(ErrorCode::AlreadyExists);
    }

    arch::interrupt_acquire(irq)?;

    // FIXME: Disable the interrupt if the following operation fails.
    let interrupt = Interrupt::new(irq)?;
    interrupts[irq as usize] = Some(interrupt.clone());

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let slot = handle_table.reserve()?;

    let handle = Handle::new(interrupt, HandleRight::ALL);
    let id = slot.insert(handle);
    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_interrupt_acknowledge(
    current: &SharedRef<Thread>,
    a0: usize,
) -> Result<SyscallResult, ErrorCode> {
    let handle_id = HandleId::from_raw(a0);

    let process = current.process();
    let handle_table = process.handle_table().lock();
    let interrupt = handle_table
        .get::<Interrupt>(handle_id)?
        .authorize(HandleRight::WRITE)?;

    arch::interrupt_acknowledge(interrupt.irq);

    Ok(SyscallResult::Return(0))
}
