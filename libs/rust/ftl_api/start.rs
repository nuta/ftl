use ftl_types::syscall::VsyscallPage;

use crate::syscall::set_vsyscall;

extern "Rust" {
    fn main();
}

#[no_mangle]
pub fn start_ftl_api(vsyscall_page: *const VsyscallPage) {
    set_vsyscall(unsafe { &*vsyscall_page });
    unsafe {
        main();
    }
}