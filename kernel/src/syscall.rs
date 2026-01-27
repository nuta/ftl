use core::slice;

use ftl_types::error::ErrorCode;
use ftl_types::syscall::SYS_CONSOLE_WRITE;
use ftl_types::syscall::SYS_PCI_LOOKUP;

use crate::arch;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;
use crate::thread::return_to_user;

fn do_syscall(
    thread: &SharedRef<Thread>,
    n: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<usize, ErrorCode> {
    match n {
        SYS_CONSOLE_WRITE => {
            let s = unsafe { slice::from_raw_parts(a0 as *const u8, a1) };
            match core::str::from_utf8(s) {
                Ok(s) => println!("[user] {}", s.trim_ascii_end()),
                Err(_) => println!("[user] invalid UTF-8"),
            }

            Ok(0)
        }
        #[cfg(target_arch = "x86_64")]
        SYS_PCI_LOOKUP => arch::sys_pci_lookup(&thread, a0, a1, a2, a3),
        _ => {
            println!("unknown syscall: {}", n);
            Err(ErrorCode::UnknownSyscall)
        }
    }
}

pub extern "C" fn syscall_handler(
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    _a4: usize,
    n: usize,
) -> ! {
    let current = &arch::get_cpuvar().current_thread;
    let thread = current.thread();
    let result = do_syscall(&thread, n, a0, a1, a2, a3);
    unsafe { current.set_syscall_result(result) };
    return_to_user();
}
