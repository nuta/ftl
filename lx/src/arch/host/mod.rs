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

    pub fn retval(&self) -> isize {
        todo!()
    }

    pub fn set_retval(&mut self, _retval: isize) {
        todo!()
    }

    pub unsafe fn enter_signal(&mut self, _signal: usize, _handler: usize, _restorer: usize) {
        todo!()
    }
}

pub extern "C" fn syscall_handler() -> ! {
    todo!()
}

pub extern "C" fn restore_regs() -> ! {
    todo!()
}
