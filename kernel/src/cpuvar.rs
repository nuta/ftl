use crate::arch;
use crate::thread::CurrentThread;

pub struct CpuVar {
    pub arch: arch::CpuVar,
    // Note: Do not wrap this field. The assembly assumes it is pointer to
    //       `arch::Thread`.
    pub current_thread: CurrentThread,
}

pub fn init(cpu_id: usize) {
    arch::set_cpuvar(
        cpu_id,
        CpuVar {
            arch: arch::CpuVar::new(cpu_id),
            current_thread: CurrentThread::new(),
        },
    );
}
