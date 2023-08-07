use core::{
    cell::RefCell,
    ops::Deref,
    ptr::{self, addr_of},
    sync::atomic::AtomicU8, mem::size_of,
};

use crate::{
    arch::{read_cpulocal_base, write_cpulocal_base},
    memory::allocate_pages,
};

#[repr(C)]
pub struct Cpuvar {
    pub panic_counter: AtomicU8,
}

impl Cpuvar {
    pub const fn new() -> Cpuvar {
        Cpuvar {
            panic_counter: AtomicU8::new(0),
        }
    }
}

pub const KERNEL_STACK_SIZE: usize = 1 * 1024 * 1024;

pub fn cpuvar() -> &'static Cpuvar {
    unsafe {
        &*(read_cpulocal_base() as *const Cpuvar)
    }
}

/// Initializes the CPU-local variables. This function must be called
/// after the memory allocator is initialized and in each CPU initialization.
pub fn init() {
    // Allocate the percpu area and the per-CPU kernel stack.
    let allocated =
        allocate_pages(KERNEL_STACK_SIZE + size_of::<Cpuvar>()).expect("failed to allocate percpu area");

    // First KERNEL_STACK_SIZE bytes are for the per-CPU kernel stack.
    let percpu = allocated.offset(KERNEL_STACK_SIZE);

    // SAFETY: `percpu` is a valid pointer to the percpu area.
    unsafe {
        ptr::write(percpu.as_mut_ptr(), Cpuvar::new());
    }

    write_cpulocal_base(percpu.as_usize());
}
