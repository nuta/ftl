use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\nkernel panic: {info}");
    loop {}
}
