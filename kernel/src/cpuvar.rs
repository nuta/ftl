use core::cell::RefCell;

use crate::arch;
use crate::shared_ref::SharedRef;
use crate::thread::CurrentThread;
use crate::thread::Thread;

pub struct CpuVar {
    pub arch: arch::CpuVar,
    pub idle_thread: SharedRef<Thread>,
    pub current_thread: RefCell<CurrentThread>,
}

pub fn init() {
    let idle_thread = Thread::new_idle().unwrap();
    arch::set_cpuvar(
        0, // FIXME:
        CpuVar {
            arch: arch::CpuVar::new(),
            current_thread: RefCell::new(CurrentThread::new(idle_thread.clone())),
            idle_thread,
        },
    );
}
