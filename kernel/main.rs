#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(fn_align)]
#![feature(offset_of)]
#![feature(custom_test_frameworks)]
#![feature(const_mut_refs)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[macro_use]
mod print;

mod address;
mod arch;
mod bootfs;
mod cpu_local;
mod giant_lock;
mod memory;
mod memory_pool;
mod object;
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
    memory::init();
    cpu_local::init_percpu();

    #[cfg(test)]
    {
        test_main();
        unreachable!();
    }

    memory::allocate_all_pages();

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
        size_of::<ref_count::SharedRefInner::<u8>>(),
        core::mem::align_of::<ref_count::SharedRefInner::<u8>>()
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

    arch::shutdown();
}
