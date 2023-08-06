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

use elf_utils::{Elf, PhdrType};
use essentials::alignment::align_up;

use crate::{
    address::{PAddr, UAddr},
    arch::{PageTable, PAGE_SIZE},
    memory_pool::memory_pool_mut,
    process::Process,
    ref_count::{SharedRef, UniqueRef},
};

#[macro_use]
mod print;

mod address;
mod arch;
mod bootfs;
mod cpu_local;
mod giant_lock;
mod memory;
mod memory_pool;
mod panic;
mod process;
mod ref_count;
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
    cpu_local::init_percpu();

    #[cfg(test)]
    {
        test_main();
        unreachable!();
    }

    let mut v = alloc::vec::Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    println!("{:#?}", v);

    println!("\n\n\x1b[1;35mHello from Rust World!\x1b[0m\n\n");

    use core::mem::size_of;
    println!("size_of::<Process>(): {}", size_of::<process::Process>());
    println!("size_of::<Handle>(): {}", size_of::<process::Handle>());
    println!("size_of::<Thread>(): {}", size_of::<thread::Thread>());
    println!(
        "size_of::<SharedRef<u8>>(): {}",
        size_of::<ref_count::SharedRef<u8>>()
    );
    println!(
        "size_of::<SharedRefHeader>(): {} (align={})",
        size_of::<ref_count::SharedObject::<u8>>(),
        core::mem::align_of::<ref_count::SharedObject::<u8>>()
    );

    // unsafe extern "C" fn first_user_program() {
    //     core::arch::asm!("nop; li a0, 0xc0be; ecall; li a0, 0xbeef; ecall; li a0, 0xdead; ecall");
    // }

    // *(InterruptCounter.borrow_mut()) = 456;
    // println!("intr: {}", *InterruptCounter.borrow());

    // use alloc::boxed::Box;
    // let t =
    //     Box::new(arch::Thread::new(first_user_program as *const () as usize));
    // arch::Thread::set_current_thread(t);
    // arch::Thread::switch_test();

    let mut pagetable =
        memory::allocate_and_initialize(PAGE_SIZE, |pool, vaddr| {
            UniqueRef::new(
                pool.initialize_page_table(vaddr, PAGE_SIZE).unwrap(),
            )
            .unwrap()
        });

    let mut fs = bootfs::Bootfs::load();
    let file = fs.find_by_name("hello").unwrap();

    // Map recursively.
    let elf = Elf::parse(file.data).expect("failed to parse the ELF file");
    for phdr in elf.phdrs {
        if phdr.p_type != PhdrType::Load {
            continue;
        }

        let mut off = 0;
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

    let process = memory::allocate_and_initialize(PAGE_SIZE, |pool, vaddr| {
        pool.initialize_process(vaddr, PAGE_SIZE, pagetable)
            .unwrap()
    });

    memory::allocate_all_pages();

    arch::shutdown();
}
