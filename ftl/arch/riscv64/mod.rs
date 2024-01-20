use core::{
    arch::asm,
    hint,
    mem::size_of,
    sync::atomic::{AtomicBool, Ordering},
};

mod sbi;

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

static GIANT_LOCK: AtomicBool = AtomicBool::new(false);

pub fn giant_lock() {
    while GIANT_LOCK
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        hint::spin_loop();
    }
}

pub fn giant_unlock() {
    GIANT_LOCK.store(false, Ordering::Release);
}

pub fn owns_giant_lock() -> bool {
    // TODO: Check owner CPU ID
    GIANT_LOCK.load(Ordering::Relaxed)
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
