#![no_std]
#![no_main]

extern crate alloc;

use ftl::application::Event;
use ftl::application::EventLoop;
use ftl::application::OpenCompleter;
use ftl::application::ReplyEvent;
use ftl::application::RequestEvent;
use ftl::borrow::ToOwned;
use ftl::channel::Buffer;
use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::handle::Handleable;
use ftl::prelude::*;
use ftl::rc::Rc;

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

    pub fn read(&mut self, completer: &OpenCompleter) -> Result<(&str, &str), ErrorCode> {
        let len = completer.read_uri(0, self.buf.as_mut_slice())?;
        let uri = core::str::from_utf8(&self.buf[..len]).map_err(|_| ErrorCode::InvalidArgument)?;
        match uri.split_once(':') {
            Some((scheme, rest)) => Ok((scheme, rest)),
            None => Err(ErrorCode::InvalidArgument),
        }
    }
}

enum Service {
    Waiting { waiters: VecDeque<OpenCompleter> },
    Registered { server_ch: Rc<Channel> },
}

#[ftl::main]
fn main() {
    info!("Hello from bootstrap!");
    let mut eventloop = EventLoop::new().unwrap();
    let initfs = InitFs::from_start_info();
    loader::create_initfs_apps(&mut eventloop, &initfs);

    let mut uri_reader = UriReader::new();
    let mut services = HashMap::new();
    let mut opening = HashMap::new();
    loop {
        match eventloop.wait() {
            Event::Request(RequestEvent::Open { completer }) => {
                let (scheme, service_name) = match uri_reader.read(&completer) {
                    Ok(parsed_uri) => parsed_uri,
                    Err(error) => {
                        completer.error(error);
                        continue;
                    }
                };

                match scheme {
                    "connect" => {
                        match services.get_mut(service_name) {
                            Some(Service::Waiting { waiters }) => {
                                waiters.push_back(completer);
                            }
                            Some(Service::Registered { server_ch }) => {
                                let msg = Message::Open {
                                    uri: Buffer::String(format!("connect:{}", service_name)),
                                };

                                if let Err(err) = server_ch.send(msg) {
                                    warn!("failed to send open message to server: {:?}", err);
                                    continue;
                                }

                                opening.insert(server_ch.handle().id(), completer);
                            }
                            None => {
                                let mut waiters = VecDeque::new();
                                waiters.push_back(completer);
                                services
                                    .insert(service_name.to_owned(), Service::Waiting { waiters });
                            }
                        }
                    }
                    "register" => {
                        let (our_ch, their_ch) = match Channel::new() {
                            Ok(ch) => ch,
                            Err(error) => {
                                completer.error(error);
                                continue;
                            }
                        };

                        let our_ch = Rc::new(our_ch);
                        let service = Service::Registered {
                            server_ch: our_ch.clone(),
                        };

                        if let Some(service) = services.insert(service_name.to_owned(), service) {
                            let Service::Waiting { mut waiters } = service else {
                                unreachable!();
                            };

                            for waiter in waiters.drain(..) {
                                let msg = Message::Open {
                                    uri: Buffer::String(format!("connect:{}", service_name)),
                                };

                                if let Err(err) = our_ch.send(msg) {
                                    warn!("failed to send open message to waiter: {:?}", err);
                                    continue;
                                }

                                opening.insert(our_ch.handle().id(), waiter);
                            }
                        }

                        trace!("registered service: {}", service_name);
                        eventloop.add_channel(our_ch.clone()).unwrap();
                        completer.complete(their_ch);
                    }
                    _ => {
                        completer.error(ErrorCode::InvalidArgument);
                    }
                }
            }
            Event::Reply(ReplyEvent::Open { ch, new_ch, .. }) => {
                let Some(completer) = opening.remove(&ch.handle().id()) else {
                    warn!("unexpected open reply from {:?}", ch.handle().id());
                    continue;
                };

                completer.complete(new_ch);
            }
            event => warn!("unhandled event: {:?}", event),
        }
    }
}
