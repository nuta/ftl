#![no_std]
#![no_main]

use core::cmp::min;
use core::mem::size_of;

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::InitContext;
use ftl::application::OpenCompleter;
use ftl::application::ReadCompleter;
use ftl::application::WriteCompleter;
use ftl::arch::min_page_size;
use ftl::channel::Channel;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::interrupt::Interrupt;
use ftl::log::*;
use ftl::prelude::*;
use ftl::rc::Rc;
use ftl::service::Service;
use ftl_utils::alignment::align_up;
use ftl_virtio::ChainEntry;
use ftl_virtio::VirtQueue;
use ftl_virtio::VirtioPci;
use ftl_virtio::virtio_pci::DeviceType;
use ftl_virtio::virtqueue;

#[repr(C, packed)]
struct VirtioNetHdr {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
}

const MIN_ETH_FRAME: usize = 60;
const RX_BUFFER_SIZE: usize = 1514 + size_of::<VirtioNetHdr>();
const TX_BUFFER_SIZE: usize = RX_BUFFER_SIZE;

#[derive(Debug)]
enum Error {
    DmaBufAlloc(ErrorCode),
    VirtQueueFull,
}

struct OngoingRx {
    vaddr: usize,
    paddr: usize,
}

struct OngoingTx {
    vaddr: usize,
    paddr: usize,
    size: usize,
}

struct PendingWrite {
    completer: WriteCompleter,
    data: Vec<u8>,
}

enum State {
    Packet,
    Mac,
}

struct Main {
    virtio: VirtioPci,
    rxq: VirtQueue,
    txq: VirtQueue,
    ongoing_rxs: Vec<Option<OngoingRx>>,
    ongoing_txs: Vec<Option<OngoingTx>>,
    free_txs: Vec<OngoingTx>,
    pending_reads: VecDeque<ReadCompleter>,
    pending_writes: VecDeque<PendingWrite>,
    rx_queue: VecDeque<Vec<u8>>,
    mac: [u8; 6],
    states: HashMap<HandleId, State>,
}

impl Application for Main {
    fn init(ctx: &mut InitContext) -> Self {
        trace!("starting...");

        let prober = VirtioPci::probe(DeviceType::Network).unwrap();

        // Initialize virtio device
        const VIRTIO_NET_F_MAC: u32 = 1 << 5;
        let device_features = prober.read_guest_features();
        assert!(
            device_features & VIRTIO_NET_F_MAC != 0,
            "MAC feature not supported"
        );

        // Only advertise features we actually support.
        let guest_features = device_features & VIRTIO_NET_F_MAC;
        let (virtio, interrupt) = prober.finish(guest_features);

        ctx.add_interrupt(Rc::new(interrupt)).unwrap();

        // Read MAC address
        let mut mac = [0u8; 6];
        for i in 0..6 {
            mac[i] = virtio.read_device_config8(i as u16);
        }
        trace!(
            " MAC address: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );

        let mut rxq = virtio.setup_virtqueue(0).unwrap();
        let txq = virtio.setup_virtqueue(1).unwrap();

        // Initialize pending_rxs with None for each possible descriptor
        let mut pending_rxs: Vec<Option<OngoingRx>> = Vec::with_capacity(rxq.queue_size());
        for _ in 0..rxq.queue_size() {
            pending_rxs.push(None);
        }

        let mut pending_txs: Vec<Option<OngoingTx>> = Vec::with_capacity(txq.queue_size());
        for _ in 0..txq.queue_size() {
            pending_txs.push(None);
        }

        // Allocate RX buffers.
        for _ in 0..min(rxq.queue_size(), 16) {
            let mut vaddr = 0usize;
            let mut paddr = 0usize;
            ftl::dmabuf::sys_dmabuf_alloc(4096, &mut vaddr, &mut paddr).unwrap();

            // Add buffer to RX queue (device writes to it)
            let head = rxq
                .push(&[ChainEntry::Write {
                    paddr: paddr as u64,
                    len: RX_BUFFER_SIZE as u32,
                }])
                .unwrap();

            // Track which buffer is associated with this descriptor
            pending_rxs[head.0 as usize] = Some(OngoingRx { vaddr, paddr });
        }

        virtio.notify(&rxq);
        trace!("RX buffers prepared");

        // Complete virtio initialization.
        trace!("virtio device initialized");

        let service = Service::register("ethernet").unwrap();
        ctx.add_service(service).unwrap();

        Self {
            virtio,
            rxq,
            txq,
            ongoing_rxs: pending_rxs,
            ongoing_txs: pending_txs,
            free_txs: Vec::new(),
            pending_reads: VecDeque::new(),
            pending_writes: VecDeque::new(),
            rx_queue: VecDeque::new(),
            mac,
            states: HashMap::new(),
        }
    }

    fn open(&mut self, ctx: &mut Context, completer: OpenCompleter) {
        let state = self.states.get(&ctx.handle_id()).unwrap();
        match state {
            State::Mac => {
                completer.error(ErrorCode::InvalidArgument);
            }
            State::Packet => {
                let mut buf = [0u8; 64];
                let len = match completer.read_uri(0, &mut buf) {
                    Ok(len) => len,
                    Err(error) => {
                        completer.error(error);
                        return;
                    }
                };

                let Ok(uri) = core::str::from_utf8(&buf[..len]) else {
                    completer.error(ErrorCode::InvalidArgument);
                    return;
                };

                if uri != "ethernet:mac" {
                    completer.error(ErrorCode::InvalidArgument);
                    return;
                }

                let (our_ch, their_ch) = match Channel::new() {
                    Ok(pair) => pair,
                    Err(error) => {
                        completer.error(error);
                        return;
                    }
                };

                let our_ch = Rc::new(our_ch);
                if let Err(error) = ctx.add_channel(our_ch.clone()) {
                    completer.error(error);
                    return;
                }

                self.states.insert(our_ch.handle().id(), State::Mac);
                completer.complete(their_ch);
            }
        }
    }

    fn irq(&mut self, _ctx: &mut Context, interrupt: &Rc<Interrupt>, _irq: u8) {
        let isr = self.virtio.read_isr();
        if isr.virtqueue_updated() {
            // Process received packets.
            let header_len = size_of::<VirtioNetHdr>();
            while let Some(used) = self.rxq.pop() {
                let Some(rx) = self.ongoing_rxs[used.head.0 as usize].take() else {
                    trace!("missing a RX request for {:?}", used.head);
                    continue;
                };

                trace!("received packet: {} bytes", used.total_len);
                let total_len = used.total_len as usize;
                if total_len > header_len {
                    let payload_len = min(
                        total_len - header_len,
                        RX_BUFFER_SIZE.saturating_sub(header_len),
                    );
                    let mut packet = vec![0u8; payload_len];
                    unsafe {
                        let payload_ptr = (rx.vaddr + header_len) as *const u8;
                        core::ptr::copy_nonoverlapping(
                            payload_ptr,
                            packet.as_mut_ptr(),
                            payload_len,
                        );
                    }
                    self.rx_queue.push_back(packet);
                }

                // Re-add the buffer to the RX queue.
                let chain = &[ChainEntry::Write {
                    paddr: rx.paddr as u64,
                    len: RX_BUFFER_SIZE as u32,
                }];
                let head = self.rxq.push(chain).unwrap();
                self.ongoing_rxs[head.0 as usize] = Some(rx);
            }

            self.virtio.notify(&self.rxq);

            self.poll_reads();
            self.poll_writes();
        }

        interrupt.acknowledge().unwrap();
    }

    fn read(&mut self, ctx: &mut Context, completer: ReadCompleter, _offset: usize, len: usize) {
        let state = self.states.get(&ctx.handle_id()).unwrap();
        match state {
            State::Packet => {
                self.pending_reads.push_back(completer);
                self.poll_reads();
            }
            State::Mac => {
                let slice = &self.mac[..min(len, self.mac.len())];
                match completer.write_data(0, slice) {
                    Ok(len) => {
                        completer.complete(len);
                    }
                    Err(error) => {
                        trace!("failed to write MAC: {:?}", error);
                        completer.error(error);
                    }
                };
            }
        }
    }

    fn write(&mut self, ctx: &mut Context, completer: WriteCompleter, _offset: usize, len: usize) {
        let state = self.states.get(&ctx.handle_id()).unwrap();
        match state {
            State::Mac => {
                completer.error(ErrorCode::Unsupported);
            }
            State::Packet => {
                let mut data = vec![0u8; len];
                let read_len = match completer.read_data(0, &mut data) {
                    Ok(len) => len,
                    Err(error) => {
                        trace!("failed to read tx data: {:?}", error);
                        completer.error(error);
                        return;
                    }
                };

                if read_len == 0 {
                    trace!("no data to write");
                    completer.complete(0);
                    return;
                }

                data.truncate(read_len);
                self.pending_writes
                    .push_back(PendingWrite { completer, data });
                self.poll_writes();
            }
        }
    }

    fn connected(&mut self, ctx: &mut Context, ch: Channel) {
        self.states.insert(ch.handle().id(), State::Packet);
        ctx.add_channel(ch).unwrap();
    }
}

impl Main {
    fn poll_reads(&mut self) {
        while !self.pending_reads.is_empty() && !self.rx_queue.is_empty() {
            let packet = self.rx_queue.pop_front().unwrap();
            let completer = self.pending_reads.pop_front().unwrap();
            let write_len = match completer.write_data(0, &packet) {
                Ok(len) => len,
                Err(error) => {
                    trace!("failed to write rx data: {:?}", error);
                    completer.error(error);
                    continue;
                }
            };

            completer.complete(write_len);
        }
    }

    fn poll_writes(&mut self) {
        while let Some(pending) = self.pending_writes.pop_front() {
            match self.send_packet(&pending.data) {
                Ok(()) => pending.completer.complete(pending.data.len()),
                Err(Error::VirtQueueFull) => {
                    self.pending_writes.push_front(pending);
                    break;
                }
                Err(error) => {
                    trace!("failed to send tx: {:?}", error);
                    pending.completer.error(ErrorCode::Unreachable);
                }
            }
        }
    }

    fn send_packet(&mut self, data: &[u8]) -> Result<(), Error> {
        while let Some(used) = self.txq.pop() {
            // TODO: Merge into allocate_tx_buffer
            if let Some(buf) = self.ongoing_txs[used.head.0 as usize].take() {
                self.free_txs.push(buf);
            }
        }

        let header_len = size_of::<VirtioNetHdr>();
        let payload_len = data.len().max(MIN_ETH_FRAME);
        let total_len = header_len + payload_len;
        let tx = self.allocate_tx_buffer(total_len)?;

        unsafe {
            let hdr_ptr = tx.vaddr as *mut VirtioNetHdr;
            hdr_ptr.write(VirtioNetHdr {
                flags: 0,
                gso_type: 0,
                hdr_len: 0,
                gso_size: 0,
                csum_start: 0,
                csum_offset: 0,
            });

            let payload_ptr = (tx.vaddr + header_len) as *mut u8;
            core::ptr::write_bytes(payload_ptr, 0, payload_len);
            core::ptr::copy_nonoverlapping(data.as_ptr(), payload_ptr, data.len());
        }

        let head = match self.txq.push(&[
            ChainEntry::Read {
                paddr: tx.paddr as u64,
                len: header_len as u32,
            },
            ChainEntry::Read {
                paddr: (tx.paddr + header_len) as u64,
                len: payload_len as u32,
            },
        ]) {
            Ok(head) => head,
            Err(virtqueue::FullError) => {
                self.free_txs.push(tx);
                return Err(Error::VirtQueueFull);
            }
        };
        self.ongoing_txs[head.0 as usize] = Some(tx);
        self.virtio.notify(&self.txq);
        Ok(())
    }

    fn allocate_tx_buffer(&mut self, min_size: usize) -> Result<OngoingTx, Error> {
        if let Some(index) = self.free_txs.iter().position(|buf| buf.size >= min_size) {
            return Ok(self.free_txs.swap_remove(index));
        }

        let size = align_up(TX_BUFFER_SIZE.max(min_size), min_page_size());
        let mut vaddr = 0usize;
        let mut paddr = 0usize;
        ftl::dmabuf::sys_dmabuf_alloc(size, &mut vaddr, &mut paddr).map_err(Error::DmaBufAlloc)?;
        Ok(OngoingTx { vaddr, paddr, size })
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
