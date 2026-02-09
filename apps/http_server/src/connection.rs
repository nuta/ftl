use core::ops::ControlFlow;

use ftl::channel::Buffer;
use ftl::channel::Message;
use ftl::collections::vec_deque::VecDeque;
use ftl::prelude::format;
use ftl::prelude::vec::Vec;
use httparse::EMPTY_HEADER;
use httparse::Request;
use httparse::Status;

const MAX_HEADERS: usize = 64;

pub enum Connection {
    ReadingHeaders { read_buf: Vec<u8> },
    WritingResponse { chunks: VecDeque<Buffer> },
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
                        let chunks = process_request(req);
                        *self = Self::WritingResponse { chunks };
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
        let Self::WritingResponse { chunks } = self else {
            return None;
        };

        let Some(data) = chunks.pop_front() else {
            *self = Self::Completed;
            return None;
        };

        Some(Message::Write { offset: 0, data })
    }
}

fn process_request(req: Request) -> VecDeque<Buffer> {
    let (status, body) = match (req.method, req.path) {
        (Some("GET"), Some(path)) if path == "/" => (200, include_bytes!("index.html").as_slice()),
        _ => (404, b"file not found".as_slice()),
    };

    let headers = format!(
        "HTTP/1.1 {status} OK\r\nServer: FTL\r\nContent-Length: {}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n",
        body.len()
    );

    let mut chunks = VecDeque::with_capacity(2);
    chunks.push_back(Buffer::String(headers));
    chunks.push_back(Buffer::Static(body));
    chunks
}
