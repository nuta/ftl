use ftl_types::thread::ExitReason;

unsafe extern "Rust" {
    fn main();
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    unsafe {
        main();
    }

    crate::syscall::thread_exit(ExitReason::Success);
}
