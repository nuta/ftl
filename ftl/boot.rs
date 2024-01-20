use crate::arch;

pub fn boot() -> ! {
    println!("\nFTL - Faster Than \"L\"\n");

    loop {
        arch::idle();
    }
}
