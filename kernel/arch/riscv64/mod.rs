use core::{arch::asm, mem::offset_of, mem::size_of};

use alloc::{boxed::Box, sync::Arc};

use crate::{
    allocator::alloc_pages,
    fiber::Fiber,
    lock::Mutex,
    scheduler::{Scheduler, GLOBAL_SCHEDULER},
};

mod sbi;

#[inline(always)]
fn set_sscratch(value: usize) {
    unsafe {
        asm!("csrw sscratch, {}", in(reg) value);
    }
}

#[inline(always)]
fn get_sscratch() -> usize {
    let value: usize;
    unsafe {
        asm!("csrr {}, sscratch", out(reg) value);
    }
    value
}

#[repr(C)]
pub struct CpuVar {
    pub context: *mut Context,
    pub current: Arc<Mutex<Fiber>>,
    pub idle: Arc<Mutex<Fiber>>,
}

pub fn init() {
    let idle = Arc::new(Mutex::new(Fiber::new_idle()));
    let cpuvar = CpuVar {
        context: core::ptr::null_mut(),
        current: idle.clone(),
        idle,
    };

    let cpuvar_ptr = Box::leak(Box::new(cpuvar));
    set_sscratch(cpuvar_ptr as *mut CpuVar as usize);
}

pub fn cpuvar_ref() -> &'static CpuVar {
    let sscratch = get_sscratch();
    let cpuvar = sscratch as *const CpuVar;
    unsafe { &*cpuvar }
}

// FIXME: Implement RefCell-like runtime borrow checker
pub fn cpuvar_mut() -> &'static mut CpuVar {
    let sscratch = get_sscratch();
    let cpuvar = sscratch as *mut CpuVar;
    unsafe { &mut *cpuvar }
}

extern "C" fn switch_to_next() -> ! {
    Scheduler::switch_to_next(GLOBAL_SCHEDULER.lock());
}

/// # Why `#[naked]`?
///
/// - To get the correct return address from `ra`. `#[naked]` prevents inlining.
/// - To eliminate the needless prologue.
#[naked]
pub extern "C" fn yield_cpu() {
    unsafe {
        asm!(
            r#"
                csrr a0, sscratch
                ld a0, {context_offset}(a0)

                sd ra, {ra_offset}(a0)
                sd sp, {sp_offset}(a0)
                sd s0, {s0_offset}(a0)
                sd s1, {s1_offset}(a0)
                sd s2, {s2_offset}(a0)
                sd s3, {s3_offset}(a0)
                sd s4, {s4_offset}(a0)
                sd s5, {s5_offset}(a0)
                sd s6, {s6_offset}(a0)
                sd s7, {s7_offset}(a0)
                sd s8, {s8_offset}(a0)
                sd s9, {s9_offset}(a0)
                sd s10, {s10_offset}(a0)
                sd s11, {s11_offset}(a0)
                j {switch_to_next}
            "#,
            context_offset = const offset_of!(CpuVar, context),
            ra_offset = const offset_of!(Context, ra),
            sp_offset = const offset_of!(Context, sp),
            s0_offset = const offset_of!(Context, s0),
            s1_offset = const offset_of!(Context, s1),
            s2_offset = const offset_of!(Context, s2),
            s3_offset = const offset_of!(Context, s3),
            s4_offset = const offset_of!(Context, s4),
            s5_offset = const offset_of!(Context, s5),
            s6_offset = const offset_of!(Context, s6),
            s7_offset = const offset_of!(Context, s7),
            s8_offset = const offset_of!(Context, s8),
            s9_offset = const offset_of!(Context, s9),
            s10_offset = const offset_of!(Context, s10),
            s11_offset = const offset_of!(Context, s11),
            switch_to_next = sym switch_to_next,
            options(noreturn)
        )
    }
}

/// Restores an in-kernel Fiber context from ssctrach.
// #[naked]
pub extern "C" fn restore_context() -> ! {
    unsafe {
        asm!(
            r#"
                csrr a0, sscratch
                ld a0, {context_offset}(a0)

                ld ra, {ra_offset}(a0)
                ld sp, {sp_offset}(a0)
                ld s0, {s0_offset}(a0)
                ld s1, {s1_offset}(a0)
                ld s2, {s2_offset}(a0)
                ld s3, {s3_offset}(a0)
                ld s4, {s4_offset}(a0)
                ld s5, {s5_offset}(a0)
                ld s6, {s6_offset}(a0)
                ld s7, {s7_offset}(a0)
                ld s8, {s8_offset}(a0)
                ld s9, {s9_offset}(a0)
                ld s10, {s10_offset}(a0)
                ld s11, {s11_offset}(a0)
                ret
            "#,
            context_offset = const offset_of!(CpuVar, context),
            ra_offset = const offset_of!(Context, ra),
            sp_offset = const offset_of!(Context, sp),
            s0_offset = const offset_of!(Context, s0),
            s1_offset = const offset_of!(Context, s1),
            s2_offset = const offset_of!(Context, s2),
            s3_offset = const offset_of!(Context, s3),
            s4_offset = const offset_of!(Context, s4),
            s5_offset = const offset_of!(Context, s5),
            s6_offset = const offset_of!(Context, s6),
            s7_offset = const offset_of!(Context, s7),
            s8_offset = const offset_of!(Context, s8),
            s9_offset = const offset_of!(Context, s9),
            s10_offset = const offset_of!(Context, s10),
            s11_offset = const offset_of!(Context, s11),
            options(noreturn)
        )
    }
}

#[no_mangle]
#[naked]
extern "C" fn kernel_entry() -> ! {
    unsafe {
        asm!(
            r#"
                mv fp, zero
                mv ra, s1
                mv a0, s2
                ret
            "#,
            options(noreturn)
        )
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Context {
    ra: usize,
    sp: usize,
    s0: usize,
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
}

impl Context {
    pub fn new_idle() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
        }
    }

    pub fn new_kernel(pc: usize, arg: usize) -> Self {
        let stack_size = 64 * 1024;
        let sp_bottom = alloc_pages(stack_size / 4096).expect("failed to allocate stack");
        let sp = sp_bottom + stack_size;
        Self {
            ra: kernel_entry as usize,
            sp,
            s0: 0, // fp
            s1: pc,
            s2: arg,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
        }
    }
}

pub fn idle() {
    unsafe {
        asm!("wfi");
    }
}

pub fn hang() -> ! {
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

pub fn backtrace<F>(mut callback: F)
where
    F: FnMut(usize),
{
    extern "C" {
        static __kernel_start: u8;
    }

    #[repr(C, packed)]
    pub struct StackFrame {
        fp: u64,
        ra: u64,
    }

    let mut fp: u64;
    let mut ra: u64;
    unsafe {
        asm!(r#"
                mv {}, fp
                mv {}, ra
            "#,
            out(reg) fp,
            out(reg) ra,
        );
    }

    for i in 0..16 {
        let kernel_start = unsafe { &__kernel_start as *const _ as u64 };
        if ra < kernel_start || fp < kernel_start {
            break;
        }

        if i > 0 {
            callback(ra as usize);
        }

        unsafe {
            let frame = fp.saturating_sub(size_of::<StackFrame>() as u64) as *const StackFrame;
            fp = (*frame).fp;
            ra = (*frame).ra;
        }
    }
}
