use crate::memory;

pub fn boot() -> ! {
    memory::init();

    let mut v = alloc::vec::Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);

    println!("v: {:?}", v);

    panic!("booted successfully");
}
