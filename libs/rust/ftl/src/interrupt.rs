use core::fmt;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_INTERRUPT_ACKNOWLEDGE;
use ftl_types::syscall::SYS_INTERRUPT_ACQUIRE;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall1;

pub struct Interrupt {
    handle: OwnedHandle,
    irq: u8,
}

impl Interrupt {
    pub fn acquire(irq: u8) -> Result<Self, ErrorCode> {
        let handle = syscall1(SYS_INTERRUPT_ACQUIRE, irq as usize)?;
        Ok(Self {
            handle: OwnedHandle::from_raw(HandleId::from_raw(handle)),
            irq,
        })
    }

    pub fn acknowledge(&self) -> Result<(), ErrorCode> {
        syscall1(SYS_INTERRUPT_ACKNOWLEDGE, self.handle.id().as_usize())?;
        Ok(())
    }

    pub fn irq(&self) -> u8 {
        self.irq
    }
}

impl Handleable for Interrupt {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}

impl fmt::Debug for Interrupt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Interrupt").field("irq", &self.irq).finish()
    }
}
