#![no_std]
#![no_main]

extern crate ftl_api;

#[no_mangle]
pub fn main(console_write: fn(&[u8])) {
    loop {
        console_write(b"Hello, world!\n");
    }
}
