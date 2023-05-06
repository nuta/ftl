use crate::{asm, sbi};

pub struct Bencher {
    name: &'static str,
}

impl Bencher {
    pub fn run<F>(&self, mut f: F)
    where
        F: FnMut(),
    {
        let start = asm::rdcycle();
        let end = asm::rdcycle();
        let rdcycle_latency = end - start;

        let mut results = [0u64; 32];
        for i in 0..results.len() {
            let start = asm::rdcycle();
            f();
            let end = asm::rdcycle();
            // results[i] = end - start - rdcycle_latency; FIXME:
            results[i] = end - start;
        }

        let avg = results.iter().sum::<u64>() / results.len() as u64;
        println!("{} ... {} cycles (avg)", self.name, avg);
    }
}

#[test_case]
fn test_println() {
    println!("test_println output");
}

#[test_case]
fn test_bench() {
    let bencher = Bencher { name: "test_bench" };
    bencher.run(|| {
        asm::rdcycle();
    });
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("running tests!");
    for test in tests {
        test();
    }

    println!("done!");
    unsafe {
        sbi::shutdown();
    }
}
