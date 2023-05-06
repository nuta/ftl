#![cfg(test)]

use core::hint::black_box;

use proc_macros::test;

use crate::arch;

struct Benchmark {
    cpu_cycles_avg: usize,
}

#[test]
fn test_case123() {
    println!("test_case output");
}

pub struct Testing {
    name: &'static str,
    bench: Option<Benchmark>,
}

impl Testing {
    fn new() -> Testing {
        Self {
            name: "",
            bench: None,
        }
    }

    pub fn set_name(&mut self, name: &'static str) {
        self.name = name;
    }

    pub fn bench_iter<F>(&mut self, mut f: F)
    where
        F: FnMut(),
    {
        let start = arch::read_cpu_cycles();
        let end = arch::read_cpu_cycles();
        let read_cpu_cycles_latency = end - start;

        let mut results = [0usize; 32];
        for i in 0..results.len() {
            let start = arch::read_cpu_cycles();
            black_box(f());
            let end = arch::read_cpu_cycles();
            results[i] = end - start - read_cpu_cycles_latency;
        }

        let avg = results.iter().sum::<usize>() / results.len();
        self.bench = Some(Benchmark {
            cpu_cycles_avg: avg,
        });
    }
}

pub fn test_runner(tests: &[&dyn Fn(&mut Testing)]) {
    println!("running tests!");
    for test in tests {
        let mut testing = Testing::new();

        black_box(test(&mut testing));

        if let Some(Benchmark { cpu_cycles_avg }) = testing.bench {
            println!("{} ... {} cycles (avg)", testing.name, cpu_cycles_avg);
        } else {
            println!("{} ... ok", testing.name);
        }
    }

    println!("done!");
    unsafe {
        arch::shutdown();
    }
}
