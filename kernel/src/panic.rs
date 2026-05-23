#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("kernel panic: {info}");
    loop {}
}
