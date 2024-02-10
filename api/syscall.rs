mod kernel {
    pub fn console_write(s: &[u8]) {
        ftl_kernel::arch::console_write(s);
    }

    pub fn yield_cpu() {
        ftl_kernel::arch::yield_cpu();
    }
}

pub use kernel::*;
