use core::arch::asm;
use core::arch::global_asm;

use ftl_types::address::PAddr;
use ftl_types::address::VAddr;

mod backtrace;
mod cpuvar;
mod thread;

global_asm!(include_str!("interrupt.S"));

pub use backtrace::backtrace;
pub use cpuvar::cpuvar;
pub use cpuvar::set_cpuvar;
pub use cpuvar::CpuVar;
use ftl_types::error::FtlError;
pub use thread::yield_cpu;
pub use thread::Thread;

pub const PAGE_SIZE: usize = 4096;
pub const NUM_CPUS_MAX: usize = 8;

pub fn paddr2vaddr(paddr: PAddr) -> Option<VAddr> {
    // Identical mapping.
    Some(VAddr::from_nonzero(paddr.as_nonzero()))
}

pub fn vaddr2paddr(vaddr: VAddr) -> Option<PAddr> {
    // Identical mapping.
    Some(PAddr::from_nonzero(vaddr.as_nonzero()))
}

pub fn halt() -> ! {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

pub fn idle() -> ! {
    loop {
        unsafe {
            asm!("msr daifclr, #2");
            asm!("wfi");
        }
    }
}

pub fn console_write(bytes: &[u8]) {
    let ptr: *mut u8 = 0x9000000 as *mut u8;
    for byte in bytes {
        unsafe {
            core::ptr::write_volatile(ptr, *byte);
        }
    }
}

#[no_mangle]
extern "C" fn arm64_handle_exception() {
    panic!("unhandled exception");
}

#[no_mangle]
extern "C" fn handle_syscall() {
    panic!("handle_syscall");
}

struct Handler {
    entry: extern "C" fn(usize),
    arg: usize,
}

// TODO: Clear when the process exits.
static INTERRUPT_HANDLER: spin::Mutex<Option<Handler>> = spin::Mutex::new(None);

pub fn set_interrupt_handler(pc: usize, arg: usize) -> Result<(), FtlError> {
    let mut guard = INTERRUPT_HANDLER.lock();
    if guard.is_some() {
        return Err(FtlError::AlreadyExists);
    }

    *guard = Some(Handler {
        entry: unsafe { core::mem::transmute::<usize, extern "C" fn(usize)>(pc) },
        arg,
    });

    Ok(())
}

#[no_mangle]
extern "C" fn arm64_handle_interrupt() {
    println!("interrupt!");

    let guard = INTERRUPT_HANDLER.lock();
    if let Some(ref handler) = *guard {
        println!("calling interrupt handler...");
        (handler.entry)(handler.arg);
    }
}

extern "C" {
    static arm64_exception_vector: [u8; 128 * 16];
}

pub fn init() {
    unsafe {
        asm!("msr vbar_el1, {}", in(reg) &arm64_exception_vector as *const _ as u64);
    }
}
