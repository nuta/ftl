use arrayvec::ArrayVec;
use ftl_utils::byte_size::ByteSize;

use crate::arch;
use crate::cpuvar;
use crate::cpuvar::CpuId;
use crate::memory;
use crate::process::Process;
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

#[no_mangle]
fn thread_entry(thread_id: usize) {
    let ch = char::from_u32(('A' as usize + thread_id) as u32).unwrap();
    for i in 0.. {
        println!("{}: {}", ch, i);
        for _ in 0..0x100000 {}
        arch::yield_cpu();
    }
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

    Thread::spawn_kernel(thread_entry, 0);
    Thread::spawn_kernel(thread_entry, 1);
    Thread::spawn_kernel(thread_entry, 2);
    Thread::spawn_kernel(thread_entry, 3);

    let proc = Process::create();

    let hello_elf = include_bytes!("../build/apps/hello.elf");
    let elf = ftl_elf::Elf::parse(hello_elf).unwrap();
    println!("ELF: {:?}", elf);
    let mut program_space = alloc::vec![0u8; 1024 * 1024];
    let entry_addr = program_space.as_ptr() as usize + ( elf.ehdr.e_entry as usize);
    for phdr in elf.phdrs {
        if phdr.p_type == ftl_elf::PhdrType::Load {
            let mem_offset = phdr.p_vaddr as usize;
            let file_offset = phdr.p_offset as usize;
            let file_copy_len = phdr.p_filesz as usize;
            program_space[mem_offset..mem_offset + file_copy_len].copy_from_slice(&hello_elf[file_offset..file_offset + file_copy_len]);
            let zeroed_len = phdr.p_memsz as usize - phdr.p_filesz as usize;
            program_space[mem_offset + file_copy_len..mem_offset + file_copy_len + zeroed_len].fill(0);
        }
    }
    println!("program_space: {:016x}", program_space.as_ptr() as usize);
    println!("entry_addr:    {:016x}", entry_addr);
    let entry: *const fn() = unsafe { core::mem::transmute(entry_addr) };
    println!("entry:         {:016x}", entry as usize);
    unsafe { (*entry)() };

    arch::yield_cpu();

    println!("kernel is ready!");
    arch::halt();
}
