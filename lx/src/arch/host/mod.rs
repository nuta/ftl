#[derive(Clone, Copy)]
pub struct SyscallFrame {
    pub nr: usize,
}

impl SyscallFrame {
    pub fn nr(&self) -> usize {
        self.nr
    }

    pub fn arg0(&self) -> usize {
        todo!()
    }

    pub fn arg1(&self) -> usize {
        todo!()
    }

    pub fn arg2(&self) -> usize {
        todo!()
    }

    pub fn arg3(&self) -> usize {
        todo!()
    }

    pub fn arg4(&self) -> usize {
        todo!()
    }

    pub fn arg5(&self) -> usize {
        todo!()
    }

    pub fn set_retval(&mut self, _retval: isize) {
        todo!()
    }

    pub fn sp(&self) -> usize {
        todo!()
    }

    pub fn set_sp(&mut self, _sp: usize) {
        todo!()
    }
}

pub struct FaultFrame {
    pub rip: usize,
    pub fault: ftl_types::thread::Fault,
    pub addr: usize,
    pub info: usize,
}

pub const FAULT_REGS_SIZE: usize = 0;

pub extern "C" fn syscall_handler() -> ! {
    let mut frame = SyscallFrame { nr: 0 };
    crate::syscall::handle_syscall(&mut frame as *mut SyscallFrame);
    todo!()
}

pub extern "C" fn fault_handler() -> ! {
    let mut frame = FaultFrame {
        rip: 0,
        fault: ftl_types::thread::Fault::PageFault,
        addr: 0,
        info: 0,
    };
    crate::fault::handle_fault(&mut frame as *mut FaultFrame);
    todo!()
}

pub extern "C" fn restore_regs() -> ! {
    todo!()
}

#[derive(Clone, Copy)]
pub struct UserContext;

impl UserContext {
    pub fn from_syscall_frame(_frame: &SyscallFrame) -> Self {
        todo!()
    }

    pub fn sp(&self) -> usize {
        todo!()
    }

    pub unsafe fn enter_signal(
        &mut self,
        _sp: usize,
        _handler: usize,
        _restorer: usize,
        _info: crate::types::signal::SigInfo,
        _stack: crate::types::signal::StackT,
        _sigmask: crate::signal::SignalSet,
    ) {
        todo!()
    }

    pub unsafe fn return_from_signal(_sp: usize) -> (Self, crate::signal::SignalSet) {
        todo!()
    }

    pub fn restore(&self) -> ! {
        todo!()
    }
}
