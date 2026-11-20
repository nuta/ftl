use std::process::ExitCode;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

pub fn main(_args: &[String]) -> ExitCode {
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .and_then(|runtime| runtime.block_on(serve()));

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("httpd: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn serve() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:80").await?;
    println!("HTTP server listening on port 80");

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(error) = handle(stream).await {
                eprintln!("httpd: {error}");
            }
        });
    }
}

async fn handle(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buf = [0; 4096];
    let mut len = 0;
    loop {
        let read_len = stream.read(&mut buf[len..]).await?;
        if read_len == 0 {
            return Ok(());
        }

        len += read_len;
        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut req = httparse::Request::new(&mut headers);
        if req
            .parse(&buf[..len])
            .map_err(std::io::Error::other)?
            .is_partial()
        {
            continue;
        }

        let (status, content_type, body): (_, _, &[u8]) = match (req.method, req.path) {
            (Some("GET"), Some("/" | "/index.html")) => {
                ("200 OK", "text/html", include_bytes!("index.html"))
            }
            (Some("GET"), Some("/hill.webp")) => {
                ("200 OK", "image/webp", include_bytes!("hill.webp"))
            }
            _ => ("404 Not Found", "text/html", include_bytes!("404.html")),
        };

        let header = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nX-Powered-By: FTL\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes()).await?;
        return stream.write_all(body).await;
    }
}
