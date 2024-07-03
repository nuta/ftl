pub struct Context {}

pub struct Thread {
    pub(super) context: Context,
}

impl Thread {
    pub fn new_idle() -> Thread {
        todo!()
    }

    pub fn new_kernel(pc: usize, arg: usize) -> Thread {
        todo!()
    }

    pub fn resume(&self) -> ! {
        todo!()
    }
}

pub fn yield_cpu() {
}
