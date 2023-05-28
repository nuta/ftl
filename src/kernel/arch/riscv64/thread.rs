use riscv::registers::{Sstatus, SstatusFlags};

use super::switch::switch_to_user;

#[derive(Default, Debug)]

pub struct Context {
    pub pc: u64,
    pub sstatus: u64,
    pub ra: u64,
    pub sp: u64,
    pub gp: u64,
    pub tp: u64,
    pub t0: u64,
    pub t1: u64,
    pub t2: u64,
    pub t3: u64,
    pub t4: u64,
    pub t5: u64,
    pub t6: u64,
    pub a0: u64,
    pub a1: u64,
    pub a2: u64,
    pub a3: u64,
    pub a4: u64,
    pub a5: u64,
    pub a6: u64,
    pub a7: u64,
    pub s0: u64,
    pub s1: u64,
    pub s2: u64,
    pub s3: u64,
    pub s4: u64,
    pub s5: u64,
    pub s6: u64,
    pub s7: u64,
    pub s8: u64,
    pub s9: u64,
    pub s10: u64,
    pub s11: u64,
}

impl Context {
    pub fn new_user(pc: u64) -> Context {
        // TODO: Shoulnd't we inherit the sstatus by reading it?
        let mut sstatus = Sstatus::read();
        // sstatus.insert(SstatusFlags::SPIE); FIXME: set in thread initialization
        sstatus.remove(SstatusFlags::SPP);

        Context {
            pc: pc,
            sstatus: sstatus.bits() as u64,
            // Other registers are set to zero.
            ..Default::default()
        }
    }
}

pub struct Thread {
    pub context: Context,
}

impl Thread {
    pub fn new(pc: usize) -> Thread {
        Thread {
            context: Context::new_user(pc as u64),
        }
    }

    pub fn set_current_thread(thread: alloc::boxed::Box<Thread>) {
        unsafe {
            CURRENT = Some(thread);
            // FIXME:
            core::arch::asm!("mv tp, {}", in(reg)
                current_thread() as *const Thread as usize
            );
        }
    }

    pub fn switch_test() -> ! {
        unsafe {
            switch_to_user(&current_thread().context);
        }
    }
}

// FIXME:
static mut CURRENT: Option<alloc::boxed::Box<Thread>> = None;

pub fn current_thread() -> &'static Thread {
    unsafe { CURRENT.as_mut().unwrap() }
}

pub fn current_thread_mut() -> &'static mut Thread {
    unsafe { CURRENT.as_mut().unwrap() }
}
