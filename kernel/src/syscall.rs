use crate::arch::get_cpuvar;
use crate::thread::return_to_user;

pub extern "C" fn handle_syscall() -> ! {
    let current = {
        let cpuvar = get_cpuvar();
        cpuvar.current_thread.thread().unwrap()
    };

    todo!("handle syscall");
    return_to_user();
}
