use core::ops::ControlFlow;

use ftl::channel::Buffer;
use ftl::channel::Message;
use ftl::prelude::vec::Vec;
use httparse::EMPTY_HEADER;
use httparse::Request;
use httparse::Status;

const MAX_HEADERS: usize = 64;
const RECV_BUFFER_SIZE: usize = 4096;

const INDEX_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 96\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<!doctype html><html><head><title>FTL</title></head><body><h1>FTL HTTP server</h1></body></html>";
const NOT_FOUND_RESPONSE: &[u8] = b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nNot Found";
const BAD_REQUEST_RESPONSE: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

pub enum Connection {
    ReadingHeaders { read_buf: Vec<u8> },
    WritingResponse { data: Option<Buffer> },
    Errored,
    Completed,
}

impl Connection {
    pub fn new() -> Self {
        Self::ReadingHeaders {
            read_buf: Vec::new(),
        }
    }

    /// Returns true if it finished.
    pub fn handle_recv(&mut self, buf: Vec<u8>) -> ControlFlow<()> {
        match self {
            Self::ReadingHeaders { read_buf } => {
                read_buf.extend_from_slice(&buf);
                let mut headers = [EMPTY_HEADER; MAX_HEADERS];
                let mut req = Request::new(&mut headers);
                match req.parse(read_buf) {
                    Ok(Status::Complete(_)) => {
                        let response = process_request(req);

                        *self = Self::WritingResponse { data: response };
                        ControlFlow::Break(())
                    }
                    Ok(Status::Partial) => {
                        // Keep reading more data.
                        ControlFlow::Continue(())
                    }
                    Err(_) => {
                        *self = Self::Errored;
                        ControlFlow::Break(())
                    }
                }
            }
            Self::WritingResponse { .. } | Self::Errored | Self::Completed => {
                // Ignore.
                ControlFlow::Break(())
            }
        }
    }

    pub fn poll_send(&mut self) -> Option<Message> {
        match self {
            Self::WritingResponse { data } => {
                if let Some(data) = data.take() {
                    *self = Self::Completed;
                    Some(Message::Write { offset: 0, data })
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

fn process_request(req: Request) -> Option<Buffer> {
    match (req.method, req.path) {
        (Some("GET"), Some(path)) if path == "/" => Some(Buffer::Static(INDEX_RESPONSE)),
        _ => Some(Buffer::Static(NOT_FOUND_RESPONSE)),
    }
}
