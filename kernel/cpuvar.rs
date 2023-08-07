use core::{
    cell::RefCell,
    mem::size_of,
    ops::Deref,
    ptr::{self, addr_of},
    sync::atomic::AtomicU8,
};

use crate::{
    arch::{read_cpuvar_addr, write_cpuvar_addr},
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
    debug_assert!(
        read_cpuvar_addr() != 0,
        "cpuvar() called before init_percpu()"
    );
    unsafe { &*(read_cpuvar_addr() as *const Cpuvar) }
}

/// Initializes the CPU-local variables and kernel stack
///
/// # Note
///
/// This function must be called:
///
/// - After the memory allocator is initialized and
/// - In each CPU initialization.
pub fn init_percpu() {
    // Allocate the percpu area and the per-CPU kernel stack.
    let allocated = allocate_pages(KERNEL_STACK_SIZE + size_of::<Cpuvar>())
        .expect("failed to allocate percpu area");

    // First KERNEL_STACK_SIZE bytes are for the per-CPU kernel stack.
    let percpu = allocated.offset(KERNEL_STACK_SIZE);

    // SAFETY: `percpu` is a valid pointer to the percpu area.
    unsafe {
        ptr::write(percpu.as_mut_ptr(), Cpuvar::new());
    }

    write_cpuvar_addr(percpu.as_usize());
}
