#![no_std]
#![no_main]
#![allow(unused)]

use core::cmp::min;
use core::mem::size_of;

use ftl::channel::Attr;
use ftl::channel::Channel;
use ftl::collections::vec_deque::VecDeque;
use ftl::driver::DmaBuf;
use ftl::driver::DmaBufPool;
use ftl::error::ErrorCode;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::eventloop::ReadCompleter;
use ftl::prelude::*;
use ftl::rc::Rc;
use ftl_virtio::VirtQueue;
use ftl_virtio::virtio_pci::DeviceType;
use ftl_virtio::virtio_pci::VirtioPci;
use ftl_virtio::virtqueue::ChainEntry;

const READ_WAITERS_MAX: usize = 16;
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

fn handle_rx(
    rxq: &mut VirtQueue<DmaBuf>,
    dmabuf: DmaBuf,
    total_len: usize,
    completer: ReadCompleter,
) {
    // Reply to the request.
    let payload = &dmabuf.as_slice()[HEADER_LEN..HEADER_LEN + total_len];
    match completer.write(0, payload) {
        Ok(len) => completer.complete(len),
        Err(error) => completer.error(error),
    }

    // Re-push the buffer to the RX queue.
    let token = rxq.reserve().unwrap();
    let chain = &[ChainEntry::Write {
        paddr: dmabuf.paddr() as u64,
        len: BUFFER_SIZE as u32,
    }];
    rxq.push(token, chain, dmabuf);
}

#[ftl::main]
fn main() {
    let mut eventloop: EventLoop<(), Rc<()>> = EventLoop::new().unwrap();

    // Initialize virtio device.
    let prober = VirtioPci::probe(DeviceType::Network).unwrap();
    let device_features = prober.read_guest_features();
    assert!(device_features & VIRTIO_NET_F_MAC != 0);

    // Only advertise features we actually support.
    let guest_features = device_features & VIRTIO_NET_F_MAC;
    let (virtio, interrupt) = prober.finish(guest_features);
    eventloop.add_interrupt(interrupt, ()).unwrap();

    // Prepare virtqueues.
    let mut rxq = virtio.setup_virtqueue(0).unwrap();
    let mut txq = virtio.setup_virtqueue(1).unwrap();

    // // Allocate RX buffers.
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

    // Read MAC address
    let mac = [
        virtio.read_device_config8(0),
        virtio.read_device_config8(1),
        virtio.read_device_config8(2),
        virtio.read_device_config8(3),
        virtio.read_device_config8(4),
        virtio.read_device_config8(5),
    ];

    // Register the service.
    let service_ch = Channel::register("ethernet").unwrap();
    eventloop.add_channel(service_ch, ()).unwrap();

    trace!("ready: mac={mac:02x?}");
    let mut read_waiters = VecDeque::new();
    loop {
        match eventloop.wait() {
            Event::Open { completer, .. } => {
                // TODO: Check the context
                let (server_ch, client_ch) = match Channel::new() {
                    Ok(pair) => pair,
                    Err(error) => {
                        error!("failed to create channel: {:?}", error);
                        completer.error(ErrorCode::OutOfResources);
                        continue;
                    }
                };

                if let Err(error) = eventloop.add_channel(server_ch, ()) {
                    error!("failed to add channel: {:?}", error);
                    completer.error(ErrorCode::OutOfResources);
                    continue;
                }

                completer.complete(client_ch);
            }
            Event::Read { completer, .. } => {
                // TODO: Check the context
                if let Some((dmabuf, total_len)) = rxq.pop() {
                    handle_rx(&mut rxq, dmabuf, total_len, completer);
                } else if read_waiters.len() > READ_WAITERS_MAX {
                    completer.error(ErrorCode::TryLater);
                } else {
                    read_waiters.push_back(completer);
                }
            }
            Event::Write { len, completer, .. } => {
                // TODO: Check the context
                let Ok(mut dmabuf) = dmabuf_pool.alloc() else {
                    completer.error(ErrorCode::OutOfResources);
                    continue;
                };

                let header_slice = &mut dmabuf.as_mut_slice()[..HEADER_LEN];
                header_slice.fill(0);

                let payload_len = min(len, PAYLOAD_SIZE_MAX);
                let payload_slice =
                    &mut dmabuf.as_mut_slice()[HEADER_LEN..HEADER_LEN + payload_len];
                if let Err(err) = completer.read(0, payload_slice) {
                    completer.error(ErrorCode::BadAccess);
                    dmabuf_pool.free(dmabuf);
                    continue;
                }

                let chain = &[ChainEntry::Read {
                    paddr: dmabuf.paddr() as u64,
                    len: (HEADER_LEN + payload_len) as u32,
                }];

                let Some(token) = txq.reserve() else {
                    completer.error(ErrorCode::BadAccess);
                    dmabuf_pool.free(dmabuf);
                    continue;
                };

                txq.push(token, chain, dmabuf);
                virtio.notify(&txq);
                completer.complete(payload_len);
            }
            Event::Getattr {
                attr, completer, ..
            } => {
                // TODO: Check the context
                if attr != Attr::MAC {
                    completer.error(ErrorCode::InvalidArgument);
                    continue;
                }

                if let Err(error) = completer.write(0, &mac) {
                    completer.error(ErrorCode::BadAccess);
                    continue;
                }

                completer.complete(mac.len());
            }
            Event::Irq { interrupt, .. } => {
                if let Err(error) = interrupt.acknowledge() {
                    warn!("failed to acknowledge interrupt: {:?}", error);
                }

                if virtio.read_isr().virtqueue_updated() {
                    while let Some((dmabuf, _total_len)) = txq.pop() {
                        dmabuf_pool.free(dmabuf);
                    }
                    virtio.notify(&txq);

                    while rxq.can_pop() && !read_waiters.is_empty() {
                        let (dmabuf, total_len) = rxq.pop().unwrap();
                        let completer = read_waiters.pop_front().unwrap();
                        handle_rx(&mut rxq, dmabuf, total_len, completer);
                    }
                }
            }
            ev => {
                warn!("unhandled event: {:?}", ev);
            }
        }
    }
}
