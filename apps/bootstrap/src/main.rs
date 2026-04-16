#![no_std]
#![no_main]
#![allow(unused)]

extern crate alloc;

use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::MessageInfo;
use ftl::channel::MessageKind;
use ftl::channel::OpenOptions;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::handle::Handleable;
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

struct PendingOpen {
    client_ch: Rc<Channel>,
    client_mid: MessageId,
}

enum Context {
    Client { ch: Rc<Channel> },
    Service { name: String },
}

enum Service {
    Waiting {
        waiters: VecDeque<PendingOpen>,
    },
    Registered {
        server_ch: Rc<Channel>,
        pending_opens: HashMap<MessageId, PendingOpen>,
    },
}

fn forward_connect(
    service_name: &str,
    server_ch: &Rc<Channel>,
    pending_opens: &mut HashMap<MessageId, PendingOpen>,
    waiter: PendingOpen,
) -> Result<(), (PendingOpen, ErrorCode)> {
    let mid = MessageId::new(1);
    let path = format!("service/{service_name}");
    pending_opens.insert(mid, waiter);

    let options = OpenOptions::CONNECT;
    if let Err(error) =
        server_ch.send(Message::Open {
            mid,
            path: path.as_bytes(),
            options,
        })
    {
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
            (Context::Client { ch }, Event::Message { info, arg1, arg2 }) => {
                match info.kind() {
                    MessageKind::OPEN => {
                        let mut buf = vec![0; info.body_len()];
                        if let Err(error) = ch.recv_body(info, &mut buf) {
                            warn!("failed to recv with body: {:?}", error);
                            continue;
                        }

                        let path = match core::str::from_utf8(&buf) {
                            Ok(path) => path,
                            Err(error) => {
                                warn!("failed to convert path to string: {:?}", error);
                                ch.reply_error(info.mid(), ErrorCode::InvalidArgument);
                                continue;
                            }
                        };

                        let Some(service_name) = path.strip_prefix("service/") else {
                            warn!("invalid path: {:?}", path);
                            ch.reply_error(info.mid(), ErrorCode::InvalidArgument);
                            continue;
                        };

                        let options = OpenOptions::from_usize(arg1);
                        if options == OpenOptions::CONNECT {
                            let waiter = PendingOpen {
                                client_ch: ch.clone(),
                                client_mid: info.mid(),
                            };

                            match services.get_mut(service_name) {
                                Some(Service::Registered {
                                    server_ch,
                                    pending_opens,
                                }) => {
                                    if let Err((waiter, error)) = forward_connect(
                                        service_name,
                                        server_ch,
                                        pending_opens,
                                        waiter,
                                    ) {
                                        warn!("failed to forward connect request: {:?}", error);
                                        waiter.client_ch.reply_error(waiter.client_mid, error);
                                    }
                                }
                                Some(Service::Waiting { waiters }) => {
                                    waiters.push_back(waiter);
                                }
                                None => {
                                    let mut waiters = VecDeque::new();
                                    waiters.push_back(waiter);
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
                                ch.reply_error(info.mid(), ErrorCode::AlreadyExists);
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
                                    ch.reply_error(info.mid(), error);
                                    continue;
                                }
                            };

                            if let Err(error) = ch.send(Message::OpenReply {
                                mid: info.mid(),
                                handle: their_ch.into_handle(),
                            }) {
                                warn!("failed to reply to service registration: {:?}", error);
                                continue;
                            }

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
                                    waiter.client_ch.reply_error(waiter.client_mid, error);
                                }
                            }
                        } else {
                            warn!("invalid options: {:?}", options);
                            ch.reply_error(info.mid(), ErrorCode::InvalidArgument);
                        }
                    }
                    _ => {
                        warn!("unhandled message: {:?}", info.kind());
                    }
                }
            }
            (Context::Service { name }, Event::Message { info, arg1, arg2 }) => {
                let Some(Service::Registered {
                    server_ch,
                    pending_opens,
                    ..
                }) = services.get_mut(name.as_str())
                else {
                    warn!("missing service state: {}", name);
                    continue;
                };

                match info.kind() {
                    MessageKind::OPEN_REPLY => {
                        let handle = match server_ch.recv_handle(info) {
                            Ok(handle) => handle,
                            Err(error) => {
                                warn!("failed to receive open reply: {:?}", error);
                                continue;
                            }
                        };

                        let Some(waiter) = pending_opens.remove(&info.mid()) else {
                            warn!("unknown open reply: mid={:?}", info.mid());
                            continue;
                        };

                        if let Err(error) = waiter.client_ch.send(Message::OpenReply {
                            mid: waiter.client_mid,
                            handle,
                        }) {
                            warn!("failed to send open reply to client: {:?}", error);
                        }
                    }
                    MessageKind::ERROR_REPLY => {
                        if let Err(error) = server_ch.recv_args(info) {
                            warn!("failed to receive error reply: {:?}", error);
                            continue;
                        }

                        let Some(waiter) = pending_opens.remove(&info.mid()) else {
                            warn!("unknown error reply: mid={:?}", info.mid());
                            continue;
                        };

                        let error = ErrorCode::from(arg1);
                        waiter.client_ch.reply_error(waiter.client_mid, error);
                    }
                    _ => {
                        warn!("unhandled service message: {:?}", info.kind());
                    }
                }
            }
            (_context, event) => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
