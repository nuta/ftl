use core::mem::size_of;

use arrayvec::ArrayVec;
use ftl_elf::Rela;
use ftl_elf::ShType;
use ftl_utils::alignment::is_aligned;
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
    let base_addr = program_space.as_ptr() as usize;
    let entry_addr = base_addr + (elf.ehdr.e_entry as usize);
    for phdr in elf.phdrs {
        if phdr.p_type == ftl_elf::PhdrType::Load {
            let mem_offset = phdr.p_vaddr as usize;
            let file_offset = phdr.p_offset as usize;
            let file_copy_len = phdr.p_filesz as usize;
            program_space[mem_offset..mem_offset + file_copy_len]
                .copy_from_slice(&hello_elf[file_offset..file_offset + file_copy_len]);
            let zeroed_len = phdr.p_memsz as usize - phdr.p_filesz as usize;
            program_space[mem_offset + file_copy_len..mem_offset + file_copy_len + zeroed_len]
                .fill(0);
        }
    }

    fn get_cstr(buffer: &[u8], offset: usize) -> Option<&str> {
        let mut len = 0;
        while let Some(&ch) = buffer.get(offset + len) {
            if ch == 0 {
                return core::str::from_utf8(&buffer[offset..offset + len]).ok();
            }
            len += 1;
        }
        None
    }

    let shstrtab_section = elf.shdrs.get(elf.ehdr.e_shstrndx as usize).expect("missing shstrtab");
    let shstrtab = unsafe {
        core::slice::from_raw_parts(
            hello_elf.as_ptr().add(shstrtab_section.sh_offset as usize),
            shstrtab_section.sh_size as usize
        )
        };
    println!("shstrtab: len={}", shstrtab.len());

    let mut rela_dyn = None;
    for shdr in elf.shdrs {
        if let Some(name) = get_cstr(shstrtab, shdr.sh_name as usize) {
            println!("shdr: name={}, type={:?}, addr={:x}, size={:x}", name, shdr.sh_type, shdr.sh_addr, shdr.sh_size);
            if name == ".rela.dyn" {
                rela_dyn = Some(shdr);
            }
        }
    }

    let rela_dyn = rela_dyn.unwrap();
    let rela_entries = unsafe {
        assert!(rela_dyn.sh_size as usize % size_of::<Rela>() == 0, "misaligned .rela_dyn size");
        core::slice::from_raw_parts(
            hello_elf.as_ptr().add(rela_dyn.sh_offset as usize) as *const Rela,
            (rela_dyn.sh_size as usize) / size_of::<Rela>()
        )
    };

    for rela in rela_entries {
        unsafe {
            let ptr = (base_addr + rela.r_offset as usize) as *mut i64;
            println!("rela: offset={:08x}, addend={:x}: ptr={:08p}", rela.r_offset, rela.r_addend, ptr);
            *ptr += (base_addr as i64) + rela.r_addend;
        };
    }

    println!("program_space: {:016x}", program_space.as_ptr() as usize);
    println!("entry_addr:    {:016x}", entry_addr);
    let entry: unsafe extern "C" fn() = unsafe { core::mem::transmute(entry_addr) };
    println!("entry:         {:016x}", entry as usize);
    unsafe {
        println!("calling entry...");
        // core::arch::asm!("123: j 123b");
        entry();
        println!("called from entry");
    }

    arch::yield_cpu();

    println!("kernel is ready!");
    arch::halt();
}
