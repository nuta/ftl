use ftl_api::thread::UpcallArg;

use crate::arch::get_cpuvar;
use crate::scheduler;

pub extern "C" fn handle_syscall() -> ! {
    let cpuvar = get_cpuvar();
    let current = cpuvar.current_thread.thread().unwrap();

    current
        .block()
        .expect("thread not runnable at syscall entry");

    cpuvar.current_thread.clear();

    current.upcall(UpcallArg::Syscall);
    drop(current);

    scheduler::return_to_user();
}
