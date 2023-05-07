use super::switch::switch_to_kernel;
use riscv::{
    registers::{Stvec, TrapMode},
    sbi,
};

#[no_mangle]
pub fn boot_kernel() {
    println!();
    unsafe {
        Stvec::write(switch_to_kernel as *const () as usize, TrapMode::Direct);
    }

    crate::kernel_main();

    sbi::shutdown();
}
