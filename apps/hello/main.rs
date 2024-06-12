#![no_std]
#![no_main]

use ftl_api::types::error::FtlError;

extern crate ftl_api;

unsafe fn syscall(
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> Result<usize, FtlError> {
    let ret;
    core::arch::asm!(
        "ecall",
        inout("a0") a0 => ret,
        in("a1") a1 ,
        in("a2") a2,
        in("a3") a3,
        in("a4") a4,
        in("a5") a5,
    );

    let err = ret as isize;
    if err < 0 {
        Err(todo!())
    } else {
        Ok(ret)
    }
}

#[no_mangle]
pub fn main() {
    for c in "Hello World from hello app!\n".chars() {
        unsafe {
            syscall(0, c as usize, 0, 0, 0, 0).unwrap();
        }
    }

    loop {
    }
}
