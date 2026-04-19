#![no_std]
#![no_main]
#![allow(unused)]

extern crate alloc;

use core::cmp::min;
use core::mem::size_of;

use ftl::channel::Attr;
use ftl::channel::Channel;
use ftl::channel::Incoming;
use ftl::channel::Message;
use ftl::channel::MessageId;
use ftl::channel::OpenOptions;
use ftl::channel::ReadCompleter;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::driver::DmaBuf;
use ftl::driver::DmaBufPool;
use ftl::error::ErrorCode;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::interrupt::Interrupt;
use ftl::prelude::*;
use ftl::rc::Rc;
use ftl::sink::Event;
use ftl::sink::Sink;
use ftl_virtio::VirtQueue;
use ftl_virtio::virtio_pci::DeviceType;
use ftl_virtio::virtio_pci::VirtioPci;
use ftl_virtio::virtqueue::ChainEntry;

const READERS_MAX: usize = 16;
const PAYLOAD_SIZE_MAX: usize = 1514;
const BUFFER_SIZE: usize = 1514 + size_of::<VirtioNetHdr>();
const HEADER_LEN: usize = size_of::<VirtioNetHdr>();
const RX_QUEUE_MAX: usize = 16;
pub const VIRTIO_NET_F_MAC: u32 = 1 << 5;

#[repr(C, packed)]
pub struct VirtioNetHdr {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
}

enum Context {
    Supervisor,
    Server,
    Client,
    Interrupt,
}

fn handle_rx(
    rxq: &mut VirtQueue<DmaBuf>,
    dmabuf: DmaBuf,
    total_len: usize,
    completer: ReadCompleter<Rc<Channel>>,
) {
    // Reply to the request.
    let payload = &dmabuf.as_slice()[HEADER_LEN..HEADER_LEN + total_len];
    completer.reply(payload);

    // Re-push the buffer to the RX queue.
    let token = rxq.reserve().unwrap();
    let chain = &[ChainEntry::Write {
        paddr: dmabuf.paddr() as u64,
        len: BUFFER_SIZE as u32,
    }];
    rxq.push(token, chain, dmabuf);
}

#[ftl::main]
fn main(supervisor_ch: Channel) {
    let sink = Sink::new().unwrap();
    sink.add(&supervisor_ch).unwrap();

    // Ask the supervisor to register the ethernet service.
    let listen_mid = MessageId::new(1);
    supervisor_ch
        .send(Message::Open {
            mid: listen_mid,
            path: b"service/ethernet",
            options: OpenOptions::LISTEN,
        })
        .unwrap();

    // Wait for the supervisor to confirm registration.
    let server_ch = loop {
        let (id, event) = sink.wait().unwrap();
        match event {
            Event::Message(peek) if id == supervisor_ch.handle().id() => {
                match Incoming::parse(&supervisor_ch, peek) {
                    Incoming::OpenReply(reply) => match reply.recv() {
                        Ok(handle) => break Channel::from_handle(handle),
                        Err(error) => panic!("failed to recv with handle: {:?}", error),
                    },
                    _ => warn!("unhandled supervisor message: {:?}", peek),
                }
            }
            _ => warn!("unhandled event during registration: {:?}", event),
        }
    };
    let server_ch = Rc::new(server_ch);

    // Initialize virtio device.
    let prober = VirtioPci::probe(DeviceType::Network).unwrap();
    let device_features = prober.read_guest_features();
    assert!(device_features & VIRTIO_NET_F_MAC != 0);

    // Only advertise features we actually support.
    let guest_features = device_features & VIRTIO_NET_F_MAC;
    let (virtio, interrupt) = prober.finish(guest_features);

    // Prepare virtqueues.
    let mut rxq = virtio.setup_virtqueue(0).unwrap();
    let mut txq = virtio.setup_virtqueue(1).unwrap();

    // Allocate RX buffers.
    let mut dmabuf_pool = DmaBufPool::new(BUFFER_SIZE);
    for _ in 0..min(rxq.queue_size(), RX_QUEUE_MAX) {
        let dmabuf = dmabuf_pool.alloc().unwrap();
        let chain = &[ChainEntry::Write {
            paddr: dmabuf.paddr() as u64,
            len: BUFFER_SIZE as u32,
        }];

        let token = rxq.reserve().unwrap();
        rxq.push(token, chain, dmabuf);
    }

    virtio.notify(&rxq);

    // Read MAC address.
    let mac = [
        virtio.read_device_config8(0),
        virtio.read_device_config8(1),
        virtio.read_device_config8(2),
        virtio.read_device_config8(3),
        virtio.read_device_config8(4),
        virtio.read_device_config8(5),
    ];

    // Watch the server channel and the device interrupt.
    sink.add(server_ch.as_ref()).unwrap();
    sink.add(&interrupt).unwrap();

    let supervisor_id = supervisor_ch.handle().id();
    let server_id = server_ch.handle().id();
    let interrupt_id = interrupt.handle().id();

    let mut contexts: HashMap<HandleId, Context> = HashMap::new();
    contexts.insert(supervisor_id, Context::Supervisor);
    contexts.insert(server_id, Context::Server);
    contexts.insert(interrupt_id, Context::Interrupt);

    let mut clients: HashMap<HandleId, Rc<Channel>> = HashMap::new();
    let mut readers: VecDeque<ReadCompleter<Rc<Channel>>> = VecDeque::new();

    trace!("ready: mac={mac:02x?}");
    trace!("registered service");

    loop {
        let (id, event) = sink.wait().unwrap();
        let context = contexts.get(&id);
        match (context, event) {
            (Some(Context::Server), Event::Message(peek)) => {
                match Incoming::parse(server_ch.clone(), peek) {
                    Incoming::Open(request) => {
                        let (our_ch, their_ch) = match Channel::new() {
                            Ok(pair) => pair,
                            Err(error) => {
                                error!("failed to create channel: {:?}", error);
                                request.reply_error(ErrorCode::OutOfResources);
                                continue;
                            }
                        };

                        if let Err(error) = sink.add(&our_ch) {
                            error!("failed to add channel to sink: {:?}", error);
                            request.reply_error(ErrorCode::OutOfResources);
                            continue;
                        }

                        let our_id = our_ch.handle().id();
                        let our_ch = Rc::new(our_ch);
                        contexts.insert(our_id, Context::Client);
                        clients.insert(our_id, our_ch);

                        request.reply(their_ch.into_handle());
                    }
                    _ => warn!("unhandled server message: {:?}", peek),
                }
            }
            (Some(Context::Client), Event::Message(peek)) => {
                let ch = clients.get(&id).unwrap().clone();
                match Incoming::parse(ch, peek) {
                    Incoming::Read(request) => {
                        let completer = match request.recv() {
                            Ok(completer) => completer,
                            Err(error) => {
                                warn!("failed to recv read: {:?}", error);
                                continue;
                            }
                        };

                        if let Some((dmabuf, total_len)) = rxq.pop() {
                            handle_rx(&mut rxq, dmabuf, total_len, completer);
                        } else if readers.len() > READERS_MAX {
                            completer.reply_error(ErrorCode::TryLater);
                        } else {
                            readers.push_back(completer);
                        }
                    }
                    Incoming::Write(request) => {
                        let Ok(mut dmabuf) = dmabuf_pool.alloc() else {
                            request.reply_error(ErrorCode::OutOfResources);
                            continue;
                        };

                        let header_slice = &mut dmabuf.as_mut_slice()[..HEADER_LEN];
                        header_slice.fill(0);

                        let payload_len = min(request.len(), PAYLOAD_SIZE_MAX);
                        let payload_slice =
                            &mut dmabuf.as_mut_slice()[HEADER_LEN..HEADER_LEN + payload_len];
                        let (_body, completer) = match request.recv(payload_slice) {
                            Ok(v) => v,
                            Err(error) => {
                                warn!("failed to recv write body: {:?}", error);
                                dmabuf_pool.free(dmabuf);
                                continue;
                            }
                        };

                        let chain = &[ChainEntry::Read {
                            paddr: dmabuf.paddr() as u64,
                            len: (HEADER_LEN + payload_len) as u32,
                        }];

                        let Some(token) = txq.reserve() else {
                            completer.reply_error(ErrorCode::BadAccess);
                            dmabuf_pool.free(dmabuf);
                            continue;
                        };

                        txq.push(token, chain, dmabuf);
                        virtio.notify(&txq);
                        completer.reply(payload_len);
                    }
                    Incoming::GetAttr(request) => {
                        match request.attr() {
                            Attr::MAC => {

                                request.reply(&mac);
                            }
                            _ => {
                                warn!("unknown attribute: {:?}", request.attr());
                                request.reply_error(ErrorCode::InvalidArgument);
                            }
                        }
                    }
                    _ => warn!("unhandled client message: {:?}", peek),
                }
            }
            (Some(Context::Interrupt), Event::Irq { .. }) => {
                if let Err(error) = interrupt.acknowledge() {
                    warn!("failed to acknowledge interrupt: {:?}", error);
                }

                if virtio.read_isr().virtqueue_updated() {
                    while let Some((dmabuf, _total_len)) = txq.pop() {
                        dmabuf_pool.free(dmabuf);
                    }
                    virtio.notify(&txq);

                    while rxq.can_pop() && !readers.is_empty() {
                        let (dmabuf, total_len) = rxq.pop().unwrap();
                        let completer = readers.pop_front().unwrap();
                        handle_rx(&mut rxq, dmabuf, total_len, completer);
                    }
                }
            }
            (_, Event::PeerClosed) => {
                if let Err(error) = sink.remove(id) {
                    warn!("failed to remove handle from sink: {:?}", error);
                }
                contexts.remove(&id);
                clients.remove(&id);
            }
            (Some(Context::Supervisor), event) => {
                warn!("unexpected supervisor event: {:?}", event);
            }
            (None, event) => {
                warn!("event for unknown handle {:?}: {:?}", id, event);
            }
            (Some(_), event) => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
