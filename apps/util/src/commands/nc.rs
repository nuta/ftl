use std::io::Write;
use std::net::TcpStream;
use std::process::ExitCode;

pub fn main(args: &[String]) -> ExitCode {
    let [host, port] = args else {
        eprintln!("usage: util nc <host> <port>");
        return ExitCode::FAILURE;
    };

    match nc(host, port) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("nc: {host}:{port}: {error}");
            ExitCode::FAILURE
        }
    }
}

fn nc(host: &str, port: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(format!("{host}:{port}"))?;
    stream.write_all(b"hello from nc\n")?;
    std::io::copy(&mut stream, &mut std::io::stdout())?;
    Ok(())
}
