use crate::arch::get_cpuvar;
use crate::scheduler;

pub extern "C" fn handle_syscall() -> ! {
    let cpuvar = get_cpuvar();
    let current = cpuvar.current_thread.thread().unwrap();

    cpuvar.current_thread.clear();
    current.handle_syscall();
    drop(current);

    scheduler::return_to_user();
}
