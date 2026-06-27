use crate::start::start_info;

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!("server panic: {info}");
    (start_info().panic)();
    loop {}
}
