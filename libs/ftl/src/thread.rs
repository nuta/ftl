use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;
use ftl_types::thread::ExitReason;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;

use crate::arch::syscall1;
use crate::arch::syscall3;
use crate::arch::syscall6;
use crate::handle::OwnedHandle;
use crate::isolate::Isolate;
use crate::vmspace::VmSpace;

pub struct Thread {
    handle: OwnedHandle,
}

impl Thread {
    pub unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn create(
        isolate: &Isolate,
        vmspace: &VmSpace,
        pc: usize,
        sp: usize,
        fault_pc: usize,
        cookie: usize,
    ) -> Result<Self, ErrorCode> {
        let id = syscall6(
            Syscall::ThreadCreate,
            isolate.handle().id().as_usize(),
            vmspace.handle().id().as_usize(),
            pc,
            sp,
            fault_pc,
            cookie,
        )?;

        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn start(&self) -> Result<(), ErrorCode> {
        syscall1(Syscall::ThreadStart, self.handle.id().as_usize())?;
        Ok(())
    }

    pub fn write_regs(&self, kind: RegsKind, regs: Regs) -> Result<(), ErrorCode> {
        syscall3(
            Syscall::ThreadWriteRegs,
            self.handle.id().as_usize(),
            kind as usize,
            &raw const regs as usize,
        )?;
        Ok(())
    }

    pub fn copy_regs_to(&self, dest: &Self, kind: RegsKind) -> Result<(), ErrorCode> {
        syscall3(
            Syscall::ThreadCopyRegs,
            self.handle.id().as_usize(),
            dest.handle.id().as_usize(),
            kind as usize,
        )?;
        Ok(())
    }
}

pub fn exit(reason: ExitReason) -> ! {
    let _ = syscall1(Syscall::ThreadExit, reason as usize);
    crate::arch::unreachable();
}
