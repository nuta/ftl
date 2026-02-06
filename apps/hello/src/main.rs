#![no_std]
#![no_main]

#[unsafe(no_mangle)]
fn main() {
    ftl::trace!("Hello World!");
}
