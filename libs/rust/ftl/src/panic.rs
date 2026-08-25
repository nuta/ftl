#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use ftl_types::thread::ExitReason;

    error!("panic: {info}");
    crate::syscall::thread_exit(ExitReason::Panic);
}
