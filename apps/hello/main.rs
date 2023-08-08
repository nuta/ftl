#![no_std]
#![no_main]

extern crate user;

unsafe fn syscall(
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> Result<usize, ()> {
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
        Err(()) // TODO:
    } else {
        Ok(ret)
    }
}

#[no_mangle]
fn main() {
    loop {
        unsafe {
            for i in 0..1000 {
                let _ = syscall(i, 0, 0, 0, 0, 0);
            }
        }
    }
}
