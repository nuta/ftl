use ftl_types::vcpu::ExitReason;

unsafe extern "Rust" {
    fn main();
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    unsafe {
        main();
    }

    crate::syscall::vcpu_exit(ExitReason::Success);
}
