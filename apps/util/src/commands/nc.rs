use std::io::Write;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::process::ExitCode;
use std::time::Duration;

const UDP_TIMEOUT: Duration = Duration::from_secs(2);

pub fn main(args: &[String]) -> ExitCode {
    let (udp, host, port) = match args {
        [host, port] => (false, host, port),
        // TODO: A good flag parser
        [flag, host, port] if flag == "-u" => (true, host, port),
        _ => {
            eprintln!("usage: util nc [-u] <host> <port>");
            return ExitCode::FAILURE;
        }
    };

    let result = if udp {
        nc_udp(host, port)
    } else {
        nc_tcp(host, port)
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("nc: {host}:{port}: {error}");
            ExitCode::FAILURE
        }
    }
}

fn nc_tcp(host: &str, port: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(format!("{host}:{port}"))?;
    stream.write_all(b"hello from nc\n")?;
    std::io::copy(&mut stream, &mut std::io::stdout())?;
    Ok(())
}

fn nc_udp(host: &str, port: &str) -> std::io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(UDP_TIMEOUT))?;
    socket.connect(format!("{host}:{port}"))?;
    socket.send(b"hello from nc\n")?;

    let mut buf = [0; 1500];
    let len = socket.recv(&mut buf)?;
    std::io::stdout().write_all(&buf[..len])?;
    Ok(())
}
