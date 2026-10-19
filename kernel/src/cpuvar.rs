use core::cell::Cell;

use ftl_types::time::MonoTime;

use crate::arch;
use crate::thread::CurrentThread;

pub struct CpuVar {
    pub arch: arch::CpuVar,
    // Note: Do not wrap this field. The assembly assumes it is pointer to
    //       `arch::Thread`.
    pub current_thread: CurrentThread,
    pub switched_at: Cell<MonoTime>,
}

pub fn init(cpu_id: usize) {
    arch::set_cpuvar(
        cpu_id,
        CpuVar {
            arch: arch::CpuVar::new(cpu_id),
            current_thread: CurrentThread::new(),
            switched_at: Cell::new(MonoTime::from_nanos(0)),
        },
    );
}
