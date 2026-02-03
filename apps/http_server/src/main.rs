#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use ftl::application::Application;
use ftl::application::Context;
use ftl::channel::Buffer;
use ftl::channel::BufferMut;
use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::handle::OwnedHandle;
use ftl::prelude::*;
use ftl::println;
use ftl::rc::Rc;
use httparse::EMPTY_HEADER;
use httparse::Request;
use httparse::Status;

const LISTEN_PORT: u16 = 80;
const READ_BUFFER_SIZE: usize = 2048;
const MAX_HEADERS: usize = 16;
const INDEX_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 96\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<!doctype html><html><head><title>FTL</title></head><body><h1>FTL HTTP server</h1></body></html>";
const NOT_FOUND_RESPONSE: &[u8] = b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nNot Found";
const BAD_REQUEST_RESPONSE: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

struct ConnState {
    ch: Rc<Channel>,
    read_buf: Vec<u8>,
    write_queue: VecDeque<Buffer>,
    read_in_flight: bool,
    write_in_flight: bool,
    closing: bool,
}

impl ConnState {
    fn new(ch: Rc<Channel>) -> Self {
        Self {
            ch,
            read_buf: Vec::new(),
            write_queue: VecDeque::new(),
            read_in_flight: false,
            write_in_flight: false,
            closing: false,
        }
    }

    fn start_read(&mut self) {
        if self.read_in_flight || self.closing {
            return;
        }

        let msg = Message::Read {
            offset: 0,
            data: BufferMut::Vec(vec![0u8; READ_BUFFER_SIZE]),
        };

        match self.ch.send(msg) {
            Ok(()) => {
                self.read_in_flight = true;
            }
            Err(error) => {
                println!("[http] failed to send read: {:?}", error);
                self.closing = true;
            }
        }
    }

    fn queue_write(&mut self, data: Buffer) {
        self.write_queue.push_back(data);
    }

    fn flush_write(&mut self) {
        if self.write_in_flight {
            return;
        }

        let Some(data) = self.write_queue.pop_front() else {
            return;
        };

        let msg = Message::Write { offset: 0, data };

        match self.ch.send(msg) {
            Ok(()) => {
                self.write_in_flight = true;
            }
            Err(error) => {
                println!("[http] failed to send write: {:?}", error);
                self.write_in_flight = false;
                self.write_queue.clear();
                self.closing = true;
            }
        }
    }

    fn should_remove(&self) -> bool {
        self.closing && !self.read_in_flight && !self.write_in_flight && self.write_queue.is_empty()
    }
}

enum ChannelState {
    Control { ch: Rc<Channel> },
    Listener { ch: Rc<Channel> },
    Conn(ConnState),
}

struct Main {
    channels: HashMap<HandleId, ChannelState>,
}

impl Main {
    fn start_listen(&mut self, ch: &Rc<Channel>) {
        let mut uri = String::from("tcp-listen:0.0.0.0:");
        let _ = write!(uri, "{}", LISTEN_PORT);

        if let Err(error) = ch.send(Message::Open {
            uri: Buffer::String(uri),
        }) {
            println!("[http] failed to request listen: {:?}", error);
        }
    }

    fn request_accept(&self, ch: &Rc<Channel>) {
        if let Err(error) = ch.send(Message::Open {
            uri: Buffer::Static(b""),
        }) {
            println!("[http] failed to request accept: {:?}", error);
        }
    }

    fn build_response(is_index: bool) -> Buffer {
        if is_index {
            Buffer::Static(INDEX_RESPONSE)
        } else {
            Buffer::Static(NOT_FOUND_RESPONSE)
        }
    }

    fn bad_request_response() -> Buffer {
        Buffer::Static(BAD_REQUEST_RESPONSE)
    }

    fn process_requests(&mut self, ch_id: HandleId) {
        let action = {
            let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&ch_id) else {
                return;
            };

            let mut headers = [EMPTY_HEADER; MAX_HEADERS];
            let mut req = Request::new(&mut headers);

            match req.parse(&conn.read_buf) {
                Ok(Status::Complete(parsed_len)) => {
                    let path = req.path.unwrap_or("/");
                    let is_index = matches!(path, "/" | "/index.html");
                    drop(req);
                    conn.read_buf.drain(0..parsed_len);
                    let response = Self::build_response(is_index);
                    conn.queue_write(response);
                    conn.closing = true;
                    true
                }
                Ok(Status::Partial) => false,
                Err(_) => {
                    conn.read_buf.clear();
                    conn.queue_write(Self::bad_request_response());
                    conn.closing = true;
                    true
                }
            }
        };

        if action {
            if let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&ch_id) {
                conn.flush_write();
            }
        }

        if let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&ch_id) {
            if !conn.closing {
                conn.start_read();
            }
        }
    }

    fn remove_conn(&mut self, ctx: &mut Context, ch_id: HandleId) {
        self.channels.remove(&ch_id);
        if let Err(error) = ctx.remove(ch_id) {
            println!("[http] failed to remove channel {:?}: {:?}", ch_id, error);
        }
    }

    fn maybe_finish_conn(&mut self, ctx: &mut Context, ch_id: HandleId) {
        let should_remove = {
            let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&ch_id) else {
                return;
            };
            conn.should_remove()
        };

        if should_remove {
            self.remove_conn(ctx, ch_id);
        }
    }
}

impl Application for Main {
    fn init(ctx: &mut Context) -> Self {
        println!("[http] starting...");
        let ch_id = HandleId::from_raw(1);
        let ch = Rc::new(Channel::from_handle(OwnedHandle::from_raw(ch_id)));
        ctx.add_channel(ch.clone()).unwrap();

        let mut app = Self {
            channels: HashMap::new(),
        };
        app.channels
            .insert(ch_id, ChannelState::Control { ch: ch.clone() });
        app.start_listen(&ch);
        app
    }

    fn open(&mut self, _ctx: &mut Context, completer: ftl::application::OpenCompleter) {
        println!("[http] unexpected open");
        completer.error(ErrorCode::Unsupported);
    }

    fn open_reply(&mut self, ctx: &mut Context, ch: &Rc<Channel>, _uri: Buffer, new_ch: Channel) {
        let ch_id = ctx.handle_id();
        let new_ch = Rc::new(new_ch);

        if let Err(error) = ctx.add_channel(new_ch.clone()) {
            println!("[http] failed to add channel: {:?}", error);
            return;
        }

        let new_id = new_ch.handle().id();
        let is_control = matches!(
            self.channels.get(&ch_id),
            Some(ChannelState::Control { .. })
        );
        let is_listener = matches!(
            self.channels.get(&ch_id),
            Some(ChannelState::Listener { .. })
        );

        if is_control {
            println!("[http] opened a listen port: {:?}", new_id);
            self.channels
                .insert(new_id, ChannelState::Listener { ch: new_ch.clone() });
            self.request_accept(&new_ch);
            return;
        }

        if is_listener {
            self.channels
                .insert(new_id, ChannelState::Conn(ConnState::new(new_ch.clone())));
            if let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&new_id) {
                conn.start_read();
            }
            self.request_accept(ch);
            return;
        }

        println!("[http] unexpected open reply on {:?}", ch_id);
    }

    fn read_reply(&mut self, ctx: &mut Context, _ch: &Rc<Channel>, buf: BufferMut, len: usize) {
        let ch_id = ctx.handle_id();

        let mut payload = None;
        let mut eof = false;
        {
            let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&ch_id) else {
                println!("[http] unexpected read reply on {:?}", ch_id);
                return;
            };

            conn.read_in_flight = false;

            if len == 0 {
                conn.closing = true;
                eof = true;
            } else {
                let data = match buf {
                    BufferMut::Vec(mut data) => {
                        let len = len.min(data.len());
                        data.truncate(len);
                        data
                    }
                    _ => {
                        println!("[http] unexpected buffer type");
                        conn.closing = true;
                        return;
                    }
                };

                if !data.is_empty() {
                    payload = Some(data);
                }
            }
        }

        if let Some(data) = payload {
            if let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&ch_id) {
                conn.read_buf.extend_from_slice(&data);
            }
            self.process_requests(ch_id);
        } else if eof {
            self.maybe_finish_conn(ctx, ch_id);
        } else if let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&ch_id) {
            if !conn.closing {
                conn.start_read();
            }
        }

        self.maybe_finish_conn(ctx, ch_id);
    }

    fn write_reply(&mut self, ctx: &mut Context, _ch: &Rc<Channel>, buf: Buffer, len: usize) {
        let ch_id = ctx.handle_id();

        if let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&ch_id) {
            conn.write_in_flight = false;
            let bytes = match buf {
                Buffer::Vec(data) => data,
                Buffer::String(data) => data.into_bytes(),
                Buffer::Static(data) => data.to_vec(),
            };
            let expected_len = bytes.len();
            if len == 0 || len > expected_len {
                println!(
                    "[http] unexpected write length {} (expected {})",
                    len, expected_len
                );
                conn.write_queue.clear();
                conn.closing = true;
            } else if len < expected_len {
                conn.write_queue
                    .push_front(Buffer::Vec(bytes[len..].to_vec()));
            }
            conn.flush_write();
        }

        self.maybe_finish_conn(ctx, ch_id);
    }

    fn peer_closed(&mut self, ctx: &mut Context, _ch: &Rc<Channel>) {
        let ch_id = ctx.handle_id();

        let should_remove = {
            let Some(ChannelState::Conn(conn)) = self.channels.get_mut(&ch_id) else {
                return;
            };
            conn.closing = true;
            conn.should_remove()
        };

        if should_remove {
            self.remove_conn(ctx, ch_id);
        }
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
