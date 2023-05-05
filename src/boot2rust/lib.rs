#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]

core::arch::global_asm!(include_str!("boot.S"));

#[macro_use]
mod print;

mod asm;
mod panic;
mod sbi;
mod switch;

#[must_use]
unsafe fn push(sp: usize, value: usize) -> usize {
    let sp = sp - core::mem::size_of::<usize>();
    (sp as *mut usize).write(value);
    sp
}

struct ThreadContext {
    sp: usize,
}

impl ThreadContext {
    fn new(entry: fn(), stack_top: usize) -> Self {
        let mut sp = stack_top;
        unsafe {
            sp = push(sp, 0); // s11
            sp = push(sp, 0); // s10
            sp = push(sp, 0); // s9
            sp = push(sp, 0); // s8
            sp = push(sp, 0); // s7
            sp = push(sp, 0); // s6
            sp = push(sp, 0); // s5
            sp = push(sp, 0); // s4
            sp = push(sp, 0); // s3
            sp = push(sp, 0); // s2
            sp = push(sp, 0); // s1
            sp = push(sp, 0); // s0
            sp = push(sp, entry as *const () as usize); // ra
        }

        Self { sp }
    }
}

static mut STACK_A: [u8; 1024 * 128] = [0u8; 1024 * 128];
static mut STACK_B: [u8; 1024 * 128] = [0u8; 1024 * 128];

static mut THREAD_A: ThreadContext = ThreadContext { sp: 0 };
static mut THREAD_B: ThreadContext = ThreadContext { sp: 0 };
static mut THREAD_BOOT: ThreadContext = ThreadContext { sp: 0 };

#[no_mangle]
pub fn rust_entry() {
    println!("\n\n\x1b[1;35mHello from Rust World!\x1b[0m\n\n");

    unsafe {
        THREAD_A = ThreadContext::new(
            entry_a,
            STACK_A.as_ptr() as usize + STACK_A.len(),
        );
        THREAD_B = ThreadContext::new(
            entry_b,
            STACK_B.as_ptr() as usize + STACK_B.len(),
        );
    }

    fn delay(cycles: u64) {
        let started = asm::rdcycle();
        loop {
            let now = asm::rdcycle();
            if now >= started + cycles {
                break;
            }
        }
    }

    fn entry_a() {
        loop {
            print!("A");
            delay(30000000);
            unsafe { switch::switch_context(&mut THREAD_A.sp, THREAD_B.sp) };
        }
    }

    fn entry_b() {
        loop {
            print!("B");
            delay(30000000);
            unsafe { switch::switch_context(&mut THREAD_B.sp, THREAD_A.sp) };
        }
    }

    unsafe { switch::switch_context(&mut THREAD_BOOT.sp, THREAD_A.sp) };

    unsafe {
        sbi::shutdown();
    }
}
