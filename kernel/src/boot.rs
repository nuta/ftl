use ftl_arrayvec::ArrayVec;

use crate::address::PAddr;
use crate::arch::paddr2vaddr;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::{self};
use crate::scheduler::SCHEDULER;
use crate::thread::Thread;
use crate::thread::return_to_user;

#[derive(Debug)]
pub struct FreeRam {
    pub base: PAddr,
    pub size: usize,
}

pub struct BootInfo {
    pub free_rams: ArrayVec<FreeRam, 8>,
}

pub fn boot(bootinfo: &BootInfo) -> ! {
    memory::init(bootinfo);

    let mut v = alloc::vec::Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    println!("v: {:?}", v);

    const STACK_SIZE: usize = 16 * 1024;
    let sp1_bottom = paddr2vaddr(PAGE_ALLOCATOR.alloc(STACK_SIZE).unwrap());
    let sp1 = sp1_bottom.as_usize() + STACK_SIZE;
    let sp2_bottom = paddr2vaddr(PAGE_ALLOCATOR.alloc(STACK_SIZE).unwrap());
    let sp2 = sp2_bottom.as_usize() + STACK_SIZE;
    let thread1 = alloc::sync::Arc::new(Thread::new(thread_entry as usize, sp1, b'A' as usize));
    let thread2 = alloc::sync::Arc::new(Thread::new(thread_entry as usize, sp2, b'B' as usize));
    SCHEDULER.push(thread1);
    SCHEDULER.push(thread2);

    return_to_user();
}

extern "C" fn thread_entry(arg: usize) -> ! {
    unsafe extern "C" {
        fn direct_syscall_handler(a0: usize);
    }

    loop {
        unsafe {
            direct_syscall_handler(arg);
        }
    }
}
