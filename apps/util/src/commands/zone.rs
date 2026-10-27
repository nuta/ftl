use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::net::Ipv4Addr;
use std::net::TcpListener;
use std::process::ExitCode;

const LOCAL_IP: Ipv4Addr = Ipv4Addr::new(10, 77, 0, 1);
const STDIO_PORT: u16 = 5000;

const FRAME_STDOUT: u8 = 1;
const FRAME_STDERR: u8 = 2;

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

    let (mut stream, _) = listener.accept()?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    loop {
        let mut header = [0; 3];
        match stream.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        }

        let type_ = header[0];
        let len = u16::from_le_bytes([header[1], header[2]]) as usize;

        let (name, buf) = match type_ {
            FRAME_STDOUT => ("stdout", &mut stdout),
            FRAME_STDERR => ("stderr", &mut stderr),
            type_ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("unknown frame type: {type_}"),
                ));
            }
        };

        let old_len = buf.len();
        buf.resize(old_len + len, 0);
        stream.read_exact(&mut buf[old_len..])?;

        let mut offset = 0;
        for (i, &byte) in buf.iter().enumerate().skip(old_len) {
            if byte == b'\n' {
                let line = String::from_utf8_lossy(&buf[offset..i]);
                println!("[zone:{name}] {line}");
                offset = i + 1;
            }
        }

        buf.drain(..offset);
    }

    if !stdout.is_empty() {
        let line = String::from_utf8_lossy(&stdout);
        println!("[zone:stdout] {line}");
    }

    if !stderr.is_empty() {
        let line = String::from_utf8_lossy(&stderr);
        println!("[zone:stderr] {line}");
    }

    Ok(())
}
