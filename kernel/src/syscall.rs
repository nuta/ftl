use crate::arch::get_cpuvar;

pub extern "C" fn handle_syscall() -> ! {
    let _current = {
        let cpuvar = get_cpuvar();
        cpuvar.current_thread.thread().unwrap()
    };

    todo!("handle syscall");
}
