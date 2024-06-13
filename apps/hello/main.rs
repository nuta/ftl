#![no_std]
#![no_main]

use ftl_api::prelude::*;

extern crate ftl_api;

#[no_mangle]
pub fn main() {
    println!("Hello World from hello app!");
    let s = String::from("1, 2, 3");
    println!("s: {}", s);
    loop {}
}
