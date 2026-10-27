use std::fs::OpenOptions;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::TcpListener;
use std::process::ExitCode;
use std::time::Duration;

const LOCAL_IP: Ipv4Addr = Ipv4Addr::new(10, 77, 0, 1);
const STDIO_PORT: u16 = 5000;

const FRAME_STDOUT: u8 = 1;
const FRAME_STDERR: u8 = 2;
const FRAME_EXIT: u8 = 3;

pub fn main(args: &[String]) -> ExitCode {
    let [init, args @ ..] = args else {
        eprintln!("usage: util zone <init> [args...]");
        return ExitCode::FAILURE;
    };

    match run(init, args) {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            eprintln!("zone: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(init: &str, args: &[String]) -> std::io::Result<u8> {
    let listener = TcpListener::bind((LOCAL_IP, STDIO_PORT))?;
    let mut spec = format!("stdio {LOCAL_IP}:{STDIO_PORT}\ninit {init}\n");
    for arg in args {
        let arg = arg.replace('\\', "\\\\").replace('\n', "\\n");
        spec.push_str(&format!("arg {arg}\n"));
    }
    spec.push_str("exec\n");

    let mut fork = OpenOptions::new().write(true).open("/lx/zone/fork")?;
    fork.write_all(spec.as_bytes())?;

    let (mut stream, _) = listener.accept()?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = loop {
        let mut header = [0; 3];
        match stream.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "connection closed without exit frame",
                ));
            }
            Err(error) => return Err(error),
        }

        let type_ = header[0];
        let value = u16::from_le_bytes([header[1], header[2]]);

        let (name, buf) = match type_ {
            FRAME_STDOUT => ("stdout", &mut stdout),
            FRAME_STDERR => ("stderr", &mut stderr),
            FRAME_EXIT => break value as u8,
            type_ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("unknown frame type: {type_}"),
                ));
            }
        };

        let old_len = buf.len();
        buf.resize(old_len + value as usize, 0);
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
    };

    // Close the connection and wait for the zone to acknowledge our FIN.
    // Othsewise, this LX will try sending FIN to close the connection
    // gracefully.
    stream.set_linger(Some(Duration::from_secs(5)))?;
    drop(stream);

    if !stdout.is_empty() {
        let line = String::from_utf8_lossy(&stdout);
        println!("[zone:stdout] {line}");
    }

    if !stderr.is_empty() {
        let line = String::from_utf8_lossy(&stderr);
        println!("[zone:stderr] {line}");
    }

    println!("[zone:exit] status {status}");

    // Closing the fork file destroys the zone.
    drop(fork);

    Ok(status)
}
