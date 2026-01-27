use crate::arch;
use crate::shared_ref::SharedRef;
use crate::thread::CurrentThread;
use crate::thread::Thread;

pub struct CpuVar {
    pub arch: arch::CpuVar,
    pub idle_thread: SharedRef<Thread>,
    // Note: Do not wrap this field. The assembly assumes it is pointer to
    //       `arch::Thread`.
    pub current_thread: CurrentThread,
}

pub fn init() {
    let idle_thread = Thread::new_idle().unwrap();
    arch::set_cpuvar(
        0, // FIXME:
        CpuVar {
            arch: arch::CpuVar::new(),
            current_thread: CurrentThread::new(idle_thread.clone()),
            idle_thread,
        },
    );
}
