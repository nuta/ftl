use core::slice;

use ftl_types::syscall::SYS_CONSOLE_WRITE;

use crate::thread::return_to_user;

pub extern "C" fn syscall_handler(
    a0: usize,
    a1: usize,
    _a2: usize,
    _a3: usize,
    _a4: usize,
    n: usize,
) -> ! {
    println!("syscall: n={}, [{:x}, {:x}]", n, a0, a1);
    match n {
        SYS_CONSOLE_WRITE => {
            let s = unsafe { slice::from_raw_parts(a0 as *const u8, a1) };
            match core::str::from_utf8(s) {
                Ok(s) => println!("[user] {}", s),
                Err(_) => println!("[user] invalid UTF-8"),
            }
        }
        _ => {
            println!("unknown syscall: {}", n);
        }
    }

    return_to_user();
}
