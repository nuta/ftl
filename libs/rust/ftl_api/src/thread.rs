use crate::handle::Handle;
use crate::start::start_info;
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

/// General-purpose register(s) for system call return value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Sysret {
    pub retval: u64,
}

pub struct Thread {
    handle: Handle,
}

impl Thread {
    pub fn create(vmspace: &VmSpace) -> crate::Result<Self> {
        let start_info = start_info();
        let handle = (start_info.thread_create)(vmspace.handle())?;
        Ok(Self { handle })
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
}
