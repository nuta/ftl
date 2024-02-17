#![no_std]

pub use ftl_kernel::arch::get_cpu_id;
pub mod callback {
    pub use ftl_kernel::arch::init_per_cpu;
    pub use ftl_kernel::arch::listen_for_hardware_interrupts;
}
