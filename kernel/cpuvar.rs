use core::{
    cell::{Ref, RefCell, RefMut},
    mem::size_of,
    ops::Deref,
    ptr::{self, addr_of},
    sync::atomic::AtomicU8,
};

use crate::{
    arch::{read_cpuvar_addr, write_cpuvar_addr},
    memory::allocate_pages,
    ref_count::SharedRef,
    thread::Thread,
};

#[repr(C)]
pub struct CpuVar {
    pub magic: u32,
    pub current_thread: Option<SharedRef<Thread>>,
}

impl CpuVar {
    pub const fn new() -> RefCell<CpuVar> {
        RefCell::new(CpuVar {
            magic: 0xc12c12,
            current_thread: None,
        })
    }
}

pub const KERNEL_STACK_SIZE: usize = 1 * 1024 * 1024;

pub fn cpuvar_refcell() -> &'static RefCell<CpuVar> {
    debug_assert!(
        read_cpuvar_addr() != 0,
        "cpuvar() called before init_percpu()"
    );

    println!("cpuvar_refcell: {:x}", read_cpuvar_addr());
    unsafe { &*(read_cpuvar_addr() as *const RefCell<CpuVar>) }
}

pub fn cpuvar() -> Ref<'static, CpuVar> {
    let guard = cpuvar_refcell().borrow();
    debug_assert_eq!(guard.magic, 0xc12c12, "invalid cpuvar magic");
    guard
}

pub fn cpuvar_mut() -> RefMut<'static, CpuVar> {
    let guard = cpuvar_refcell().borrow_mut();
    debug_assert_eq!(guard.magic, 0xc12c12, "invalid cpuvar magic");
    println!("OK cpuvar_mut");
    guard
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
    let allocated = allocate_pages(KERNEL_STACK_SIZE + size_of::<CpuVar>())
        .expect("failed to allocate percpu area");

    // First KERNEL_STACK_SIZE bytes are for the per-CPU kernel stack.
    let percpu = allocated.offset(KERNEL_STACK_SIZE);

    // SAFETY: `percpu` is a valid pointer to the percpu area.
    unsafe {
        ptr::write(percpu.as_mut_ptr(), CpuVar::new());
    }

    write_cpuvar_addr(percpu.as_usize());
}
