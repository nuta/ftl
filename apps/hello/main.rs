#![no_std]
#![no_main]

use ftl_api::types::vsyscall::VsyscallPage;

extern crate ftl_api;

#[no_mangle]
pub fn main(vsyscall: *const VsyscallPage) {
    for c in "Hello World from hello app!\n".chars() {
        unsafe {
            ((*vsyscall).entry)(0, c as isize, 0, 0, 0, 0).unwrap();
        }
    }

    loop {
    }
}
