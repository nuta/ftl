#![no_std]
#![no_main]
#![allow(unused)]

use core::cmp::min;

use ftl::arch::min_page_size;
use ftl::collections::vec_deque::VecDeque;
use ftl::driver::DmaBufPool;
use ftl::error::ErrorCode;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::eventloop::Request;
use ftl::prelude::*;
use ftl::service::Service;
use ftl_utils::alignment::align_up;
use ftl_virtio::virtio_pci::DeviceType;
use ftl_virtio::virtio_pci::VirtioPci;
use ftl_virtio::virtqueue::ChainEntry;

const VIRTIO_NET_F_MAC: u32 = 1 << 5;
const CONCURRENT_READS_LIMIT: usize = 16;
const PACKET_BUFFER_SIZE: usize = 1514;
const RX_QUEUE_MAX: usize = 16;

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
    let mut dmabuf_pool = DmaBufPool::new(align_up(PACKET_BUFFER_SIZE, min_page_size()));
    for _ in 0..min(rxq.queue_size(), RX_QUEUE_MAX) {
        let dmabuf = dmabuf_pool.alloc().unwrap();
        let chain = &[ChainEntry::Write {
            paddr: dmabuf.paddr() as u64,
            len: dmabuf.len() as u32,
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
            Event::Request(Request::Read { len, completer }) => {
                if let Some((dmabuf, _total_len)) = rxq.pop() {
                    // Reply to the request.
                    completer.complete_with(dmabuf.as_slice());

                    // Re-push the buffer to the RX queue.
                    let token = rxq.reserve().unwrap();
                    let chain = &[ChainEntry::Write {
                        paddr: dmabuf.paddr() as u64,
                        len: dmabuf.len() as u32,
                    }];
                    rxq.push(token, chain, dmabuf);
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

                // TODO: header

                if let Err(err) = completer.read_data(0, &mut dmabuf.as_mut_slice()[..len]) {
                    completer.error(err);
                    dmabuf_pool.free(dmabuf);
                    continue;
                }

                let chain = &[ChainEntry::Read {
                    paddr: dmabuf.paddr() as u64,
                    len: dmabuf.len() as u32,
                }];

                let Some(token) = txq.reserve() else {
                    completer.error(ErrorCode::TryLater);
                    dmabuf_pool.free(dmabuf);
                    continue;
                };

                txq.push(token, chain, dmabuf);
                virtio.notify(&txq);
                completer.complete(len);
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
                        let (dmabuf, _total_len) = rxq.pop().unwrap();
                        let completer = pending_reads.pop_front().unwrap();

                        completer.complete_with(dmabuf.as_slice());
                        let chain = &[ChainEntry::Write {
                            paddr: dmabuf.paddr() as u64,
                            len: dmabuf.len() as u32,
                        }];
                        let token = rxq.reserve().unwrap();
                        rxq.push(token, chain, dmabuf);
                    }
                }
            }
            ev => {
                warn!("unhandled event: {:?}", ev);
            }
        }
    }
}
