#![no_std]
#![no_main]
#![allow(unused)]

use core::cmp::min;
use core::mem::size_of;

use ftl::arch::min_page_size;
use ftl::collections::vec_deque::VecDeque;
use ftl::driver::DmaBuf;
use ftl::driver::DmaBufPool;
use ftl::error::ErrorCode;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::eventloop::ReadCompleter;
use ftl::eventloop::Request;
use ftl::prelude::*;
use ftl::service::Service;
use ftl_utils::alignment::align_up;
use ftl_virtio::VirtQueue;
use ftl_virtio::virtio_pci::DeviceType;
use ftl_virtio::virtio_pci::VirtioPci;
use ftl_virtio::virtqueue::ChainEntry;

const CONCURRENT_READS_LIMIT: usize = 16;
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
    completer.complete_with(payload);

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
    let mut eventloop = EventLoop::new().unwrap();

    // Initialize virtio device.
    let prober = VirtioPci::probe(DeviceType::Network).unwrap();
    let device_features = prober.read_guest_features();
    assert!(device_features & VIRTIO_NET_F_MAC != 0);

    // Only advertise features we actually support.
    let guest_features = device_features & VIRTIO_NET_F_MAC;
    let (virtio, interrupt) = prober.finish(guest_features);
    eventloop.add_interrupt(interrupt).unwrap();

    // Prepare virtqueues.
    let mut rxq = virtio.setup_virtqueue(0).unwrap();
    let mut txq = virtio.setup_virtqueue(1).unwrap();

    // // Allocate RX buffers.
    let mut dmabuf_pool = DmaBufPool::new(align_up(BUFFER_SIZE, min_page_size()));
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
    let mut mac = [
        virtio.read_device_config8(0),
        virtio.read_device_config8(1),
        virtio.read_device_config8(2),
        virtio.read_device_config8(3),
        virtio.read_device_config8(4),
        virtio.read_device_config8(5),
    ];

    // Register the service.
    let service = Service::register("ethernet").unwrap();
    eventloop.add_service(service).unwrap();

    trace!("ready: mac={mac:02x?}");
    let mut pending_reads = VecDeque::new();
    loop {
        match eventloop.wait() {
            Event::Request(Request::Read { len: _, completer }) => {
                if let Some((dmabuf, total_len)) = rxq.pop() {
                    handle_rx(&mut rxq, dmabuf, total_len, completer);
                } else if pending_reads.len() > CONCURRENT_READS_LIMIT {
                    completer.error(ErrorCode::TryLater);
                } else {
                    pending_reads.push_back(completer);
                }
            }
            Event::Request(Request::Write { len, completer }) => {
                let Ok(mut dmabuf) = dmabuf_pool.alloc() else {
                    warn!("failed to allocate a DMA buffer");
                    completer.error(ErrorCode::TryLater);
                    continue;
                };

                let header_slice = &mut dmabuf.as_mut_slice()[..HEADER_LEN];
                header_slice.fill(0);

                let payload_len = min(len, PAYLOAD_SIZE_MAX);
                let payload_slice =
                    &mut dmabuf.as_mut_slice()[HEADER_LEN..HEADER_LEN + payload_len];
                if let Err(err) = completer.read_data(0, payload_slice) {
                    completer.error(err);
                    dmabuf_pool.free(dmabuf);
                    continue;
                }

                let chain = &[ChainEntry::Read {
                    paddr: dmabuf.paddr() as u64,
                    len: (HEADER_LEN + payload_len) as u32,
                }];

                let Some(token) = txq.reserve() else {
                    completer.error(ErrorCode::TryLater);
                    dmabuf_pool.free(dmabuf);
                    continue;
                };

                txq.push(token, chain, dmabuf);
                virtio.notify(&txq);
                completer.complete(payload_len);
            }
            Event::Request(Request::Invoke { completer }) => {
                match completer.kind() {
                    1 => completer.complete_with(&mac),
                    _ => completer.error(ErrorCode::Unsupported),
                }
            }
            Event::Interrupt { interrupt } => {
                if virtio.read_isr().virtqueue_updated() {
                    while let Some((dmabuf, _total_len)) = txq.pop() {
                        dmabuf_pool.free(dmabuf);
                    }
                    virtio.notify(&txq);

                    while rxq.can_pop() && !pending_reads.is_empty() {
                        let (dmabuf, total_len) = rxq.pop().unwrap();
                        let completer = pending_reads.pop_front().unwrap();
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
