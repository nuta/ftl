use alloc::sync::Arc;

use ftl_types::address::PAddr;
use ftl_types::address::VAddr;

use crate::fiber::Fiber;
use crate::lock::Mutex;

pub fn paddr2vaddr(paddr: PAddr) -> Option<VAddr> {
    todo!()
}

pub struct IntrStateGuard {}

impl IntrStateGuard {
    pub fn save_and_disable_interrupts() -> Self {
        todo!()
    }
}

impl Drop for IntrStateGuard {
    fn drop(&mut self) {
        todo!()
    }
}

#[repr(C)]
pub struct CpuVar {
    pub hart_id: usize,
    pub context: *mut Context,
    pub current: Arc<Mutex<Fiber>>,
    pub idle: Arc<Mutex<Fiber>>,
}

pub fn init(cpu_id: usize) {
    todo!();
}

pub fn init_per_cpu<F: Fn(usize) + Send + 'static>(f: F) {
    todo!()
}

pub fn listen_for_hardware_interrupts<F: Fn() + Send + 'static>(f: F) {
    todo!()
}

pub fn get_cpu_id() -> usize {
    todo!()
}

pub fn cpuvar_ref() -> &'static CpuVar {
    todo!()
}

pub fn cpuvar_mut() -> &'static mut CpuVar {
    todo!()
}

pub fn yield_cpu() {
    todo!();
}

pub extern "C" fn restore_context() -> ! {
    todo!()
}

#[derive(Debug)]
pub struct Context {}

impl Context {
    pub fn zeroed() -> Self {
        todo!()
    }

    pub fn new_kernel(pc: usize, arg: usize) -> Self {
        todo!()
    }
}

pub fn idle() {
    todo!()
}

pub fn hang() -> ! {
    todo!()
}

pub fn console_write(bytes: &[u8]) {
    todo!()
}

pub fn backtrace<F>(mut callback: F)
where
    F: FnMut(usize),
{
    todo!()
}
