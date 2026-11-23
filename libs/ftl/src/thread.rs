use ftl_types::error::ErrorCode;
use ftl_types::handle::CURRENT_HSPACE;
use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;
use ftl_types::thread::ExitReason;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;

use crate::arch::syscall0;
use crate::arch::syscall1;
use crate::arch::syscall3;
use crate::arch::syscall6;
use crate::handle::OwnedHandle;
use crate::hspace::HandleSpace;
use crate::poll::Poll;
use crate::quota::Quota;
use crate::vmspace::VmSpace;

pub struct Thread {
    handle: OwnedHandle,
}

impl Thread {
    pub unsafe fn from_handle(id: HandleId) -> Self {
        let handle = OwnedHandle::new(id);
        Self { handle }
    }

    pub fn into_handle(self) -> OwnedHandle {
        self.handle
    }

    pub fn create(
        quota: &Quota,
        vmspace: &VmSpace,
        syscall_pc: usize,
        fault_pc: usize,
        system_sp: usize,
    ) -> Result<Self, ErrorCode> {
        Self::do_create(
            quota,
            CURRENT_HSPACE,
            vmspace,
            syscall_pc,
            fault_pc,
            system_sp,
        )
    }

    pub fn create_in(
        quota: &Quota,
        hspace: &HandleSpace,
        vmspace: &VmSpace,
        syscall_pc: usize,
        fault_pc: usize,
        system_sp: usize,
    ) -> Result<Self, ErrorCode> {
        let hspace_id = hspace.handle().id();
        Self::do_create(quota, hspace_id, vmspace, syscall_pc, fault_pc, system_sp)
    }

    fn do_create(
        quota: &Quota,
        hspace_id: HandleId,
        vmspace: &VmSpace,
        syscall_pc: usize,
        fault_pc: usize,
        system_sp: usize,
    ) -> Result<Self, ErrorCode> {
        let id = syscall6(
            Syscall::ThreadCreate,
            quota.handle().id().as_usize(),
            hspace_id.as_usize(),
            vmspace.handle().id().as_usize(),
            syscall_pc,
            fault_pc,
            system_sp,
        )?;

        // SAFETY: Kernel returns a valid handle.
        let this = unsafe { Self::from_handle(HandleId::new(id)) };
        Ok(this)
    }

    pub fn start(&self, pc: usize, sp: usize) -> Result<(), ErrorCode> {
        syscall3(Syscall::ThreadStart, self.handle.id().as_usize(), pc, sp)?;
        Ok(())
    }

    pub fn subscribe(&self, poll: &Poll, cookie: usize) -> Result<(), ErrorCode> {
        let thread_id = self.handle.id();
        let poll_id = poll.handle().id();
        syscall3(
            Syscall::ThreadSubscribe,
            thread_id.as_usize(),
            poll_id.as_usize(),
            cookie,
        )?;
        Ok(())
    }

    pub fn id(&self) -> HandleId {
        self.handle.id()
    }

    pub fn destroy(&self) -> Result<(), ErrorCode> {
        syscall1(Syscall::ThreadDestroy, self.handle.id().as_usize())?;
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

pub fn thread_yield() {
    let _ = syscall0(Syscall::ThreadYield);
}

pub fn exit(reason: ExitReason) -> ! {
    let _ = syscall1(Syscall::ThreadExit, reason as usize);
    crate::arch::unreachable();
}
