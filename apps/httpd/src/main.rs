//! A simple HTTP server.
use std::convert::Infallible;
use std::io;
use std::net::Ipv4Addr;
use std::net::SocketAddr;

use http_body_util::Full;
use hyper::Method;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::header;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

struct StaticFile {
    content_type: &'static str,
    body: &'static [u8],
}

const INDEX_HTML: StaticFile = StaticFile {
    content_type: "text/html",
    body: include_bytes!("../index.html"),
};

const NOT_FOUND_HTML: StaticFile = StaticFile {
    content_type: "text/html",
    body: include_bytes!("../404.html"),
};

const HILL_WEBP: StaticFile = StaticFile {
    content_type: "image/webp",
    body: include_bytes!("../hill.webp"),
};

fn respond(status: StatusCode, file: &StaticFile) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, file.content_type)
        .header("X-Powered-By", "FTL")
        .header(header::CONNECTION, "close")
        .body(Full::new(Bytes::from_static(file.body)))
        .unwrap()
}

async fn handle(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let (status, file) = match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") => (StatusCode::OK, &INDEX_HTML),
        (&Method::GET, "/hill.webp") => (StatusCode::OK, &HILL_WEBP),
        _ => (StatusCode::NOT_FOUND, &NOT_FOUND_HTML),
    };
    Ok(respond(status, file))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 80));
    let listener = TcpListener::bind(addr).await?;
    println!("HTTP server listening on port 80");

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(client) => client,
            Err(err) => {
                eprintln!("accept failed: {err}");
                continue;
            }
        };

        tokio::spawn(async move {
            let mut builder = http1::Builder::new();
            builder.keep_alive(false);
            builder.auto_date_header(false);

            let io = TokioIo::new(stream);
            if let Err(err) = builder.serve_connection(io, service_fn(handle)).await {
                eprintln!("client failed: {err}");
            }
        });
    }
}
