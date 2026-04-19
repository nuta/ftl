#![no_std]
#![no_main]
#![allow(unused)]

extern crate alloc;

use ftl::channel::Channel;
use ftl::channel::Incoming;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::MessageInfo;
use ftl::channel::MessageKind;
use ftl::channel::OpenCompleter;
use ftl::channel::OpenOptions;
use ftl::channel::OwnedCompleter;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::handle::Handleable;
use ftl::handle::OwnedHandle;
use ftl::prelude::*;
use ftl::rc::Rc;
use ftl::sink::Event;
use ftl::sink::Sink;
use ftl::string::String;
use ftl::string::ToString;

use crate::initfs::InitFs;

mod elf;
mod initfs;
mod loader;

enum Context {
    Client { ch: Rc<Channel> },
    Service { name: String },
}

enum Service {
    Waiting {
        waiters: VecDeque<OpenCompleter<Rc<Channel>>>,
    },
    Registered {
        server_ch: Rc<Channel>,
        pending_opens: HashMap<MessageId, OpenCompleter<Rc<Channel>>>,
    },
}

fn forward_connect(
    service_name: &str,
    server_ch: &Rc<Channel>,
    pending_opens: &mut HashMap<MessageId, OpenCompleter<Rc<Channel>>>,
    waiter: OpenCompleter<Rc<Channel>>,
) -> Result<(), (OpenCompleter<Rc<Channel>>, ErrorCode)> {
    let mid = MessageId::new(1);
    let path = format!("service/{service_name}");
    pending_opens.insert(mid, waiter);

    let options = OpenOptions::CONNECT;
    if let Err(error) = server_ch.send(Message::Open {
        mid,
        path: path.as_bytes(),
        options,
    }) {
        let waiter = pending_opens.remove(&mid).unwrap();
        return Err((waiter, error));
    }

    Ok(())
}

#[ftl::main]
fn main(supervisor_ch: Channel) {
    info!("Hello from bootstrap!");

    // Bootstrap is the first user process and there is no supervisor process
    // for it. Avoid accidentally dropping handle #1.
    core::mem::forget(supervisor_ch);

    let sink = Sink::new().unwrap();
    let mut contexts = HashMap::new();

    // Load apps.
    let initfs = InitFs::from_start_info();
    for file in initfs.iter() {
        trace!("loading app: {}", file.name);
        let ch = loader::load_app(&file);

        // Register channel with sink.
        let handle_id = ch.handle().id();
        sink.add(&ch).unwrap();
        contexts.insert(handle_id, Context::Client { ch: Rc::new(ch) });
    }

    // Wait for events.
    let mut services = HashMap::new();
    loop {
        let (id, event) = sink.wait().unwrap();
        let context = contexts.get(&id).unwrap();
        match (context, event) {
            (Context::Client { ch }, Event::Message(peeked)) => {
                let incoming = Incoming::parse(ch.clone() /* FIXME: avoid cloning */, peeked);
                match incoming {
                    Incoming::Open(request) => {
                        let options = request.options();
                        let mut buf = vec![0; request.path_len()];
                        let (path, completer) = match request.recv(&mut buf) {
                            Ok((path, completer)) => (path, completer),
                            Err(error) => {
                                // FIXME: Should we reply an error?
                                warn!("failed to recv with body: {:?}", error);
                                continue;
                            }
                        };

                        let path = match core::str::from_utf8(path) {
                            Ok(path) => path,
                            Err(error) => {
                                warn!("failed to convert path to string: {:?}", error);
                                completer.reply_error(ErrorCode::InvalidArgument);
                                continue;
                            }
                        };

                        let Some(service_name) = path.strip_prefix("service/") else {
                            warn!("invalid path: {:?}", path);
                            completer.reply_error(ErrorCode::InvalidArgument);
                            continue;
                        };

                        if options == OpenOptions::CONNECT {
                            match services.get_mut(service_name) {
                                Some(Service::Registered {
                                    server_ch,
                                    pending_opens,
                                }) => {
                                    if let Err((completer, error)) = forward_connect(
                                        service_name,
                                        server_ch,
                                        pending_opens,
                                        completer,
                                    ) {
                                        warn!("failed to forward connect request: {:?}", error);
                                        completer.reply_error(error);
                                    }
                                }
                                Some(Service::Waiting { waiters }) => {
                                    waiters.push_back(completer);
                                }
                                None => {
                                    let mut waiters = VecDeque::new();
                                    waiters.push_back(completer);
                                    services.insert(
                                        service_name.to_string(),
                                        Service::Waiting { waiters },
                                    );
                                }
                            }
                        } else if options == OpenOptions::LISTEN {
                            if matches!(
                                services.get(service_name),
                                Some(Service::Registered { .. })
                            ) {
                                completer.reply_error(ErrorCode::AlreadyExists);
                                continue;
                            }

                            let waiters = match services.remove(service_name) {
                                Some(Service::Waiting { waiters }) => waiters,
                                Some(Service::Registered { .. }) => unreachable!(),
                                None => VecDeque::new(),
                            };

                            let (our_ch, their_ch) = match Channel::new() {
                                Ok(pair) => pair,
                                Err(error) => {
                                    completer.reply_error(error);
                                    continue;
                                }
                            };

                            completer.reply(their_ch.into_handle());
                            let server_ch = Rc::new(our_ch);
                            sink.add(server_ch.as_ref()).unwrap();

                            contexts.insert(
                                server_ch.handle().id(),
                                Context::Service {
                                    name: service_name.to_string(),
                                },
                            );
                            services.insert(
                                service_name.to_string(),
                                Service::Registered {
                                    server_ch: server_ch.clone(),
                                    pending_opens: HashMap::new(),
                                },
                            );

                            let Service::Registered {
                                server_ch,
                                pending_opens,
                            } = services.get_mut(service_name).unwrap()
                            else {
                                unreachable!();
                            };
                            for waiter in waiters {
                                if let Err((waiter, error)) =
                                    forward_connect(service_name, server_ch, pending_opens, waiter)
                                {
                                    warn!("failed to forward queued connect request: {:?}", error);
                                    waiter.reply_error(error);
                                }
                            }
                        } else {
                            warn!("invalid options: {:?}", options);
                            completer.reply_error(ErrorCode::InvalidArgument);
                        }
                    }
                    _ => {
                        warn!("unhandled message: {:?}", peeked);
                    }
                }
            }
            (Context::Service { name }, Event::Message(peeked)) => {
                let Some(Service::Registered {
                    server_ch,
                    pending_opens,
                    ..
                }) = services.get_mut(name.as_str())
                else {
                    warn!("missing service state: {}", name);
                    continue;
                };

                match Incoming::parse(server_ch, peeked) {
                    Incoming::OpenReply(reply) => {
                        let Some(waiter) = pending_opens.remove(&reply.mid()) else {
                            warn!("unknown open reply: mid={:?}", reply.mid());
                            continue;
                        };

                        let handle = match reply.recv() {
                            Ok(handle) => handle,
                            Err(error) => {
                                warn!("failed to recv with handle: {:?}", error);
                                waiter.reply_error(error);
                                continue;
                            }
                        };

                        waiter.reply(handle);
                    }
                    Incoming::ErrorReply(reply) => {
                        let Some(waiter) = pending_opens.remove(&reply.mid()) else {
                            warn!("unknown error reply: mid={:?}", reply.mid());
                            continue;
                        };

                        waiter.reply_error(reply.error());
                    }
                    _ => {
                        warn!("unhandled service message: {:?}", peeked);
                    }
                }
            }
            (_context, event) => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
