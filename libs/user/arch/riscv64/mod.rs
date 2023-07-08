use core::arch::asm;

extern "C" {
    static __stack_top: u8;
}

#[no_mangle]
#[naked]
pub extern "C" fn start() {
    asm!(
        r#"
        mv ra, zero
        mv fp, zero
        la sp, {stack_top}
        call {main}
        "#,
        boot_kernel = sym boot_kernel,
        stack_top = sym __boot_stack_top,
        options(noreturn),
    );
}
