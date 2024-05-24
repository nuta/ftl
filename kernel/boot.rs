use arrayvec::ArrayVec;
use ftl_utils::byte_size::ByteSize;

use crate::arch;
use crate::cpuvar;
use crate::cpuvar::CpuId;
use crate::memory;
use crate::scheduler::GLOBAL_SCHEDULER;
use crate::thread::Thread;

/// A free region of memory available for software.
#[derive(Debug)]
pub struct FreeMem {
    /// The start address of the region.
    pub start: usize,
    /// The size of the region.
    pub size: ByteSize,
}

/// The boot information passed from the bootloader.
#[derive(Debug)]
pub struct BootInfo {
    pub free_mems: ArrayVec<FreeMem, 8>,
    pub dtb_addr: *const u8,
}

/// The entry point of the kernel.
pub fn boot(cpu_id: CpuId, bootinfo: BootInfo) -> ! {
    println!("\nFTL - Faster Than \"L\"\n");

    memory::init(&bootinfo);
    cpuvar::percpu_init(cpu_id);

    let mut v = alloc::vec::Vec::new();
    v.push(alloc::string::String::from("Hello, "));
    v.push(alloc::string::String::from("world!"));
    println!("alloc test: {:?}", v);

    println!("cpuvar test: CPU {}", arch::cpuvar().cpu_id);

    oops!("backtrace test");


    fn thread_entry(i: usize) {
        let ch = char::from_u32(('a' as usize + i) as u32).unwrap();
        loop {
            println!("{}", ch);
            for _ in 0..0x100000 {}
            GLOBAL_SCHEDULER.yield_cpu();
        }
    }

    Thread::spawn_kernel(&(thread_entry as fn(usize)), 0);
    Thread::spawn_kernel(&(thread_entry as fn(usize)), 1);
    GLOBAL_SCHEDULER.yield_cpu();

    println!("kernel is ready!");
    arch::halt();
}
