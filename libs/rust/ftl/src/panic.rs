use core::panic::PanicInfo;

use ftl_types::syscall::SYS_PROCESS_EXIT;

use crate::syscall;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    trace!("panic: {}", info);
    let _ = syscall::syscall0(SYS_PROCESS_EXIT);
    unreachable!();
}
