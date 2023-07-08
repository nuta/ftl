use core::arch::asm;

extern "C" {
    static __stack_top: u8;
}

#[no_mangle]
#[naked]
pub extern "C" fn start() {
    unsafe {
        asm!(
            r#"
            mv ra, zero
            mv fp, zero
            la sp, {stack_top}
            call main
            "#,
            stack_top = sym __stack_top,
            options(noreturn),
        );
    }
}
