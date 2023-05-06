use core::hint::black_box;

use proc_macros::test;
use riscv::{instructions::rdcycle, sbi};

pub struct Bencher {
    name: &'static str,
}

impl Bencher {
    pub fn iter<F>(&self, mut f: F)
    where
        F: FnMut(),
    {
        let start = rdcycle();
        let end = rdcycle();
        let rdcycle_latency = end - start;

        let mut results = [0u64; 32];
        for i in 0..results.len() {
            let start = rdcycle();
            black_box(f());
            let end = rdcycle();
            results[i] = end - start - rdcycle_latency;
        }

        let avg = results.iter().sum::<u64>() / results.len() as u64;
        println!("{} ... {} cycles (avg)", self.name, avg);
    }
}

#[test]
fn test_case123() {
    println!("test_case output");
}

pub struct Testing {
    name: &'static str,
    bencher: Bencher,
}

impl Testing {
    pub fn set_name(&mut self, name: &'static str) {
        self.name = name;
    }
}

pub fn test_runner(tests: &[&dyn Fn(&mut Testing)]) {
    println!("running tests!");
    for test in tests {
        let mut testing = &mut Testing {
            name: "",
            bencher: Bencher { name: "" },
        };
        test(testing);
        println!("{} ... ok", testing.name);
    }

    println!("done!");
    unsafe {
        sbi::shutdown();
    }
}
