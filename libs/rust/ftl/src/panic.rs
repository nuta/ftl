#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use ftl_types::vcpu::ExitReason;

    error!("server panic: {info}");
    crate::syscall::vcpu_exit(ExitReason::Panic);
}
