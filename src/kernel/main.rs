#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(fn_align)]
#![feature(offset_of)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[macro_use]
mod print;

mod arch;
mod cpu_local;
mod giant_lock;
mod memory;
mod panic;
mod test;

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
