use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[lang = "start"]
fn start<T>(main: fn() -> T, _argc: isize, _argv: *const *const u8, _: u8) -> isize {
    main();
    0
}
