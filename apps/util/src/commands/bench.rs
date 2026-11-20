use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

const PAGE_SIZE: usize = 4096;

struct Benchmark {
    name: &'static str,
    run: fn() -> std::io::Result<()>,
}

const BENCHMARKS: &[Benchmark] = &[
    Benchmark {
        name: "getpid",
        run: getpid,
    },
    Benchmark {
        name: "pipe",
        run: pipe,
    },
    Benchmark {
        name: "file-read",
        run: file_read,
    },
    Benchmark {
        name: "mmap",
        run: mmap,
    },
];

fn getpid() -> std::io::Result<()> {
    std::hint::black_box(std::process::id());
    Ok(())
}

fn pipe() -> std::io::Result<()> {
    let (mut reader, mut writer) = std::io::pipe()?;
    let mut buf = [0; 1];
    writer.write_all(b"x")?;
    reader.read_exact(&mut buf)?;
    Ok(())
}

fn file_read() -> std::io::Result<()> {
    let mut file = File::open("/proc/version")?;
    let mut buf = [0; 256];
    std::hint::black_box(file.read(&mut buf)?);
    Ok(())
}

fn mmap() -> std::io::Result<()> {
    let mut buf = vec![0u8; 64 * PAGE_SIZE];
    for page in buf.chunks_mut(PAGE_SIZE) {
        page[0] = 1;
    }
    std::hint::black_box(&buf);
    Ok(())
}

pub fn main(args: &[String]) -> ExitCode {
    let [name, iterations_str] = args else {
        eprintln!("usage: util bench <name> <iterations>");
        return ExitCode::FAILURE;
    };

    let iterations = iterations_str
        .parse::<usize>()
        .expect("iterations must be a positive integer");

    let benchmark = BENCHMARKS
        .iter()
        .find(|b| b.name == name)
        .expect("unknown benchmark: {name}");

    // Prewarming.
    for _ in 0..5 {
        if let Err(error) = (benchmark.run)() {
            eprintln!("bench: {name}: {error}");
            return ExitCode::FAILURE;
        }
    }

    let started_at = Instant::now();
    for _ in 0..iterations {
        if let Err(error) = (benchmark.run)() {
            eprintln!("bench: {name}: {error}");
            return ExitCode::FAILURE;
        }
    }
    let elapsed = started_at.elapsed();

    println!(
        "bench: {name}: iterations={iterations} elapsed_ns={}",
        elapsed.as_nanos()
    );
    ExitCode::SUCCESS
}
