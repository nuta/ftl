use alloc::sync::Arc;

use crate::handle::Handle;
use crate::start::start_info;
use crate::upcall::UpCallCtx;
use crate::upcall::Upcall;
use crate::upcall::UserData;
use crate::vmspace::VmSpace;

/// The kind of thread context.
///
/// Instead of defining a single struct for all general-purpose registers,
/// we prefer to define minimal variants, designed for each use case.
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextKind {
    SyscallArgs = 0,
    Sysret = 1,
    InitRegs = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union ContextData {
    pub syscall_args: SyscallArgs,
    pub sysret: Sysret,
    pub init_regs: InitRegs,
}

/// The initial registers for a thread.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InitRegs {
    pub pc: u64,
    pub sp: u64,
}

/// General-purpose registers for system call parameters.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SyscallArgs {
    pub n: u64,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
    pub arg4: u64,
    pub arg5: u64,
}

impl SyscallArgs {
    pub const fn zeroed() -> Self {
        Self {
            n: 0,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
        }
    }
}

/// General-purpose register(s) for system call return value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Sysret {
    pub retval: u64,
}

impl Sysret {
    pub const fn zeroed() -> Self {
        Self { retval: 0 }
    }
}

pub enum UpcallArg {
    Syscall,
    Terminated,
}

pub trait Handler: Send + Sync {
    fn syscall(&self, thread: &Thread);
    fn terminated(&self, thread: &Thread);
}

fn upcall_entry<H: Handler + 'static>(ctx: UpCallCtx, arg: UpcallArg) {
    match arg {
        UpcallArg::Syscall => {
            let user_data = unsafe { UserData::<Arc<Thread>, H>::borrow(ctx) };
            user_data.handler.syscall(&user_data.object);
        }
        UpcallArg::Terminated => {
            let user_data = unsafe { UserData::<Arc<Thread>, H>::reclaim(ctx) };
            user_data.handler.terminated(&user_data.object);
        }
    }
}

pub struct Thread {
    handle: Handle,
}

impl Thread {
    pub fn create<H: Handler + 'static>(
        vmspace: &VmSpace,
        handler: H,
    ) -> crate::Result<Arc<Thread>> {
        let start_info = start_info();

        Upcall::new(upcall_entry::<H>, handler, |upcall| {
            let handle = (start_info.thread_create)(vmspace.handle(), upcall)?;
            Ok(Arc::new(Thread { handle }))
        })
    }

    pub fn get_context(&self, kind: ContextKind, regs: &mut ContextData) -> crate::Result<()> {
        let start_info = start_info();
        (start_info.thread_get_context)(&self.handle, kind, regs)
    }

    pub fn set_context(&self, kind: ContextKind, regs: &ContextData) -> crate::Result<()> {
        let start_info = start_info();
        (start_info.thread_set_context)(&self.handle, kind, regs)
    }

    pub fn unblock(&self) -> crate::Result<()> {
        let start_info = start_info();
        (start_info.thread_unblock)(&self.handle)
    }

    /// Stops the thread permanently. Safe to call from its syscall handler.
    pub fn terminate(&self) -> crate::Result<()> {
        let start_info = start_info();
        (start_info.thread_terminate)(&self.handle)
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        // SAFETY: Handle does not implement Drop, and we won't use it after
        //         the thread_destroy call below.
        let handle = unsafe { core::ptr::read(&self.handle) };

        let start_info = start_info();
        if let Err(err) = (start_info.thread_destroy)(handle) {
            error!("failed to destroy thread: {:?}", err);
        }
    }
}
