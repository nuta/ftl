use core::arch::asm;

mod backtrace;
mod cpuvar;
mod sbi;
mod thread;

pub use backtrace::backtrace;
pub use cpuvar::cpuvar;
pub use cpuvar::set_cpuvar;
pub use cpuvar::CpuVar;
pub use thread::Thread;
pub use thread::yield_cpu;

pub const NUM_CPUS_MAX: usize = 8;

pub fn halt() -> ! {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

pub fn console_write(bytes: &[u8]) {
    for byte in bytes {
        sbi::console_putchar(*byte);
    }
}
