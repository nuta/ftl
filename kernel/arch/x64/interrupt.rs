use core::arch::{asm, global_asm};

global_asm!(include_str!("interrupt.S"));

#[no_mangle]
extern "C" fn x64_handle_interrupt() -> ! {
    todo!()
}
