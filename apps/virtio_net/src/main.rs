#![no_std]
#![no_main]

use core::cmp::min;

use ftl::collections::vec_deque::VecDeque;
use ftl::error::ErrorCode;
use ftl::eventloop::EventLoop;
use ftl::prelude::*;
use ftl::service::Service;
use ftl_virtio::dma_buf::DmaBufPool;
use ftl_virtio::virtio_pci::DeviceType;
use ftl_virtio::virtio_pci::VirtioPci;
use ftl_virtio::virtqueue::ChainEntry;

const VIRTIO_NET_F_MAC: u32 = 1 << 5;
const CONCURRENT_READS_LIMIT: usize = 16;
const PACKET_BUFFER_SIZE: usize = 1514;

#[ftl::main]
fn main() {
    let mut eventloop = EventLoop::new();

    // Initialize virtio device.
    let prober = VirtioPci::probe(DeviceType::Network).unwrap();
    let device_features = prober.read_guest_features();
    assert!(device_features & VIRTIO_NET_F_MAC != 0);

    // Only advertise features we actually support.
    let guest_features = device_features & VIRTIO_NET_F_MAC;
    let (virtio, interrupt) = prober.finish(guest_features);
    eventloop.add_interrupt(interrupt).unwrap();

    // Prepare virtqueues.
    let txq = virtio.setup_virtqueue(1).unwrap();
    let mut rxq = virtio.setup_virtqueue(0).unwrap();

    // // Allocate RX buffers.
    // let dmabuf_pool = DmaBufPool::new(PACKET_BUFFER_SIZE);
    // for _ in 0..min(rxq.queue_size(), 16) {
    //     let dmabuf = dmabuf_pool.alloc().unwrap();
    //     let chain = &[ChainEntry::Write {
    //         paddr: dmabuf.paddr() as u64,
    //         len: PACKET_BUFFER_SIZE as u32,
    //     }];
    //     rxq.push(chain).unwrap();
    // }

    // virtio.notify(&rxq);

    // // Read MAC address
    // let mut mac = [0u8; 6];
    // for i in 0..6 {
    //     mac[i] = virtio.read_device_config8(i as u16);
    // }

    // // Register the service.
    // let service = Service::register("ethernet").unwrap();
    // ctx.add_service(service).unwrap();

    // trace!("ready: mac={mac:02x?}");
    // let mut pending_reads = VecDeque::new();
    // loop {
    //     match eventloop.next() {
    //         Event::Request(Request::Read { completer }) => {
    //             match rxq.pop() {
    //                 Some(dmabuf) => {
    //                     completer.complete_with(dmabuf.as_slice());
    //                 }
    //                 None if pending_reads.len() > CONCURRENT_READS_LIMIT => {
    //                     completer.error(ErrorCode::TryLater);
    //                 }
    //                 None => {
    //                     pending_reads.push_back(completer);
    //                 }
    //             }
    //         }
    //         Event::Request(Request::Write { len, completer }) => {
    //             if !txq.can_push() {
    //                 completer.error(ErrorCode::TryLater);
    //                 continue;
    //             }

    //             let Some(dmabuf) = dmabuf_pool.alloc() else {
    //                 completer.error(ErrorCode::TryLater);
    //                 continue;
    //             };

    //             let dst = dmabuf.as_mut_slice(size_of::<VirtioNetHdr>()..len);
    //             if let Err(err) = completer.read_data(0, dst) {
    //                 completer.error(err);
    //                 dmabuf_pool.free(dmabuf);
    //                 continue;
    //             }

    //             let chain = &[ChainEntry::Read {
    //                 paddr: dmabuf.paddr() as u64,
    //                 len: PACKET_BUFFER_SIZE as u32,
    //             }];

    //             txq.push(chain).unwrap();
    //             completer.complete();
    //         }
    //         Event::Connect { ch } => {
    //             eventloop.add_channel(ch).unwrap();
    //         }
    //         Event::Interrupt { interrupt } => {
    //             if virtio.read_isr().virtqueue_updated() {
    //                 while rxq.can_pop() && !pending_reads.is_empty() {
    //                     let chain = rxq.pop().unwrap();
    //                     let completer = pending_reads.pop_front().unwrap();

    //                     // completer.complete_with(dmabuf.as_slice());
    //                     dmabuf_pool.free(dmabuf);
    //                 }

    //                 while let Some(dmabuf) = txq.pop() {
    //                     dmabuf_pool.free(dmabuf);
    //                 }
    //             }
    //         }
    //         ev => {
    //             warn!("unhandled event: {:?}", ev);
    //         }
    //     }
    // }
}
