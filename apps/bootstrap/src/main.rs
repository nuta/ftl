#![no_std]
#![no_main]

extern crate alloc;

use ftl::borrow::ToOwned;
use ftl::channel::Channel;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::eventloop::Client;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::eventloop::OpenRequest;
use ftl::eventloop::Reply;
use ftl::eventloop::Request;
use ftl::prelude::*;

use crate::initfs::InitFs;

mod elf;
mod initfs;
mod loader;

pub struct UriReader {
    buf: [u8; 256],
}

impl UriReader {
    pub fn new() -> Self {
        Self { buf: [0; 256] }
    }

    pub fn read(&mut self, request: &OpenRequest) -> Result<(&str, &str), ErrorCode> {
        let len = request.path(self.buf.as_mut_slice())?;
        let uri = core::str::from_utf8(&self.buf[..len]).map_err(|_| ErrorCode::InvalidArgument)?;
        match uri.split_once(':') {
            Some((scheme, rest)) => Ok((scheme, rest)),
            None => Err(ErrorCode::InvalidArgument),
        }
    }
}

#[derive(Debug)]
enum Cookie {
    Connect(OpenRequest),
}

enum Service {
    Waiting { waiters: VecDeque<OpenRequest> },
    Registered { server_ch: Client<Cookie> },
}

#[ftl::main]
fn main() {
    info!("Hello from bootstrap!");
    let mut eventloop = EventLoop::new().unwrap();
    let initfs = InitFs::from_start_info();
    loader::create_initfs_apps(&mut eventloop, &initfs);

    let mut uri_reader = UriReader::new();
    let mut services = HashMap::new();
    loop {
        match eventloop.wait() {
            Event::Request {
                request: Request::Open(request),
                ..
            } => {
                let (scheme, service_name) = match uri_reader.read(&request) {
                    Ok(parsed_uri) => parsed_uri,
                    Err(error) => {
                        request.reply_error(error);
                        continue;
                    }
                };

                match scheme {
                    "connect" => {
                        match services.get_mut(service_name) {
                            Some(Service::Waiting { waiters }) => {
                                waiters.push_back(request);
                            }
                            Some(Service::Registered { server_ch }) => {
                                if let Err(err) = server_ch.open(
                                    format!("connect:{}", service_name),
                                    Cookie::Connect(request),
                                ) {
                                    warn!("failed to send open message to server: {:?}", err);
                                }
                            }
                            None => {
                                let mut waiters = VecDeque::new();
                                waiters.push_back(request);
                                services
                                    .insert(service_name.to_owned(), Service::Waiting { waiters });
                            }
                        }
                    }
                    "register" => {
                        let (our_ch, their_ch) = match Channel::new() {
                            Ok(ch) => ch,
                            Err(error) => {
                                request.reply_error(error);
                                continue;
                            }
                        };

                        let server_ch = eventloop.add_channel(our_ch, ()).unwrap();
                        let service = Service::Registered {
                            server_ch: server_ch.clone(),
                        };
                        if let Some(service) = services.insert(service_name.to_owned(), service) {
                            let Service::Waiting { mut waiters } = service else {
                                unreachable!();
                            };

                            for waiter in waiters.drain(..) {
                                if let Err(err) = server_ch.open(
                                    format!("connect:{}", service_name),
                                    Cookie::Connect(waiter),
                                ) {
                                    warn!("failed to send open message to waiter: {:?}", err);
                                }
                            }
                        }

                        trace!("registered service: {}", service_name);
                        request.reply(their_ch);
                    }
                    _ => {
                        request.reply_error(ErrorCode::InvalidArgument);
                    }
                }
            }
            Event::Reply {
                reply:
                    Reply::Open {
                        cookie: Cookie::Connect(waiter),
                        new_ch,
                        ..
                    },
                ..
            } => {
                waiter.reply(new_ch);
            }
            Event::Reply {
                reply:
                    Reply::Error {
                        cookie: Cookie::Connect(waiter),
                        error,
                        ..
                    },
                ..
            } => {
                warn!("failed to connect to service: {:?}", error);
                waiter.reply_error(error);
            }
            event => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
