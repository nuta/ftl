#[derive(Clone, Copy)]
pub struct SyscallFrame {
    pub nr: usize,
    pub cookie: usize,
}

impl SyscallFrame {
    pub fn nr(&self) -> usize {
        todo!()
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

    pub fn retval(&self) -> isize {
        todo!()
    }

    pub fn set_retval(&mut self, _retval: isize) {
        todo!()
    }

    pub fn set_sp(&mut self, _sp: usize) {
        todo!()
    }

    pub unsafe fn enter_signal(&mut self, _signal: usize, _handler: usize, _restorer: usize) {
        todo!()
    }
}

pub struct FaultFrame {
    pub rip: usize,
    pub cookie: usize,
    pub fault: ftl_types::thread::Fault,
    pub addr: usize,
    pub info: usize,
}

pub extern "C" fn syscall_handler() -> ! {
    let mut frame = SyscallFrame { nr: 0, cookie: 0 };
    crate::syscall::handle_syscall(&mut frame as *mut SyscallFrame);
    todo!()
}

pub extern "C" fn fault_handler() -> ! {
    todo!()
}

pub extern "C" fn restore_regs() -> ! {
    todo!()
}
