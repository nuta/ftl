use std::io::BufRead;
use std::io::BufReader;
use std::net::Ipv4Addr;
use std::net::TcpListener;
use std::process::ExitCode;

const LOCAL_IP: Ipv4Addr = Ipv4Addr::new(10, 77, 0, 1);
const STDIO_PORT: u16 = 5000;

pub fn main(args: &[String]) -> ExitCode {
    let [init, args @ ..] = args else {
        eprintln!("usage: util zone <init> [args...]");
        return ExitCode::FAILURE;
    };

    match run(init, args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zone: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(init: &str, args: &[String]) -> std::io::Result<()> {
    let listener = TcpListener::bind((LOCAL_IP, STDIO_PORT))?;
    let mut spec = format!("stdio {LOCAL_IP}:{STDIO_PORT}\ninit {init}\n");
    for arg in args {
        let arg = arg.replace('\\', "\\\\").replace('\n', "\\n");
        spec.push_str(&format!("arg {arg}\n"));
    }
    spec.push_str("exec\n");
    std::fs::write("/lx/zone/fork", spec)?;

    let (stream, _) = listener.accept()?;
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        println!("[zone:stdout] {}", line?);
    }

    Ok(())
}
