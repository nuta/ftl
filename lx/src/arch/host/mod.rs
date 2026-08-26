pub struct SyscallFrame {
    pub nr: usize,
    pub cookie: usize,
}

impl SyscallFrame {
    pub fn arg0(&self) -> usize {
        todo!()
    }

    pub fn arg1(&self) -> usize {
        todo!()
    }

    pub fn arg2(&self) -> usize {
        todo!()
    }
}

pub extern "C" fn syscall_handler() -> ! {
    todo!()
}

pub extern "C" fn fork_child_entry() -> ! {
    todo!()
}
