#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(generic_const_exprs)]
#![feature(fn_align)]
#![feature(offset_of)]
#![feature(custom_test_frameworks)]
#![feature(const_mut_refs)]
#![feature(strict_provenance)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(unused)]
#![allow(unused_variables)]

use core::num::NonZeroUsize;

use elf_utils::{Elf, PhdrType};
use essentials::alignment::{align_up, is_aligned};

use crate::{
    address::{PAddr, UAddr},
    arch::{PageTable, PAGE_SIZE},
    process::{Process, Handle, HandleId},
    ref_count::{SharedRef, UniqueRef},
};

#[macro_use]
mod print;

mod address;
mod arch;
mod backtrace;
mod bootfs;
mod cpuvar;
mod giant_lock;
mod memory;
mod memory_pool;
mod panic;
mod process;
mod ref_count;
mod scheduler;
mod test;
mod thread;

extern crate alloc;

// cpu_local! {
//     pub static InterruptCounter: RefCell<usize> = {
//         RefCell::new(123)
//     };
// }

pub fn kernel_main() {
    println!("\nFTL - Faster Than L\n");

    println!("initializing memory...");
    memory::init();

    println!("initializing CPU local storage...");
    cpuvar::init_percpu();

    #[cfg(test)]
    {
        test_main();
        unreachable!();
    }

    println!("initializing first process...");
    let mut pagetable = memory_pool::allocate_page_table().expect("failed to allocate page table");
    // TODO:
    println!("initializing mapping kernel pages...");
    pagetable.map_kernel_pages();

    println!("loading bootfs...");
    let mut fs = bootfs::Bootfs::load();
    let file = fs.find_by_name("startup.elf").unwrap();

    // Map recursively.
    let elf = Elf::parse(file.data).expect("failed to parse startup.elf");
    for phdr in elf.phdrs {
        // SAFETY: The ELF header should be aligned to 4KiB as each bootfs file is aligned to 4KiB.
        let phdr = unsafe { core::ptr::read_unaligned(phdr) };
        if phdr.p_type != PhdrType::Load {
            continue;
        }

        assert!(is_aligned(phdr.p_vaddr as usize, PAGE_SIZE));

        let mut off = 0;
        println!("phdr: {:#x?}", phdr);
        while off < align_up(phdr.p_memsz as usize, PAGE_SIZE) {
            let page4k =
                memory::allocate_and_initialize(PAGE_SIZE, |pool, vaddr| {
                    pool.initialize_page4k(vaddr, PAGE_SIZE)
                })
                .unwrap();

            let filesz = phdr.p_filesz as usize;
            if off < filesz {
                let file_off = phdr.p_offset as usize + off;
                page4k
                    .borrow_mut()
                    .write_bytes(0, &file.data[file_off..file_off + filesz]);
            }

            pagetable.map_recursively(
                UAddr::new(phdr.p_vaddr as usize + off),
                page4k,
                phdr.readable(),
                phdr.writable(),
                phdr.executable(),
                true,
            );

            off += PAGE_SIZE;
        }
    }

    let pc = UAddr::new(elf.ehdr.e_entry as usize);
    let process = memory::allocate_and_initialize(PAGE_SIZE, |pool, vaddr| {
        pool.initialize_process(vaddr, PAGE_SIZE, pagetable)
            .unwrap()
    });
    let thread = memory::allocate_and_initialize(PAGE_SIZE, |pool, vaddr| {
        pool.initialize_thread(vaddr, PAGE_SIZE, SharedRef::inc_ref(&process), pc)
            .unwrap()
    });

    process.borrow_mut().set_handle(HandleId::new(NonZeroUsize::new(1).unwrap()), Handle::Thread(SharedRef::inc_ref(&thread)));
    thread.borrow_mut().resume();
    scheduler::add_thread(thread);

    // TODO:
    memory::allocate_all_pages();

    scheduler::yield_to_user();
}
