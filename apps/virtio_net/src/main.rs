#![no_std]
#![no_main]

use core::cmp::min;
use core::mem::MaybeUninit;
use core::mem::size_of;

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::ReadCompleter;
use ftl::application::WriteCompleter;
use ftl::channel::Channel;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::handle::HandleId;
use ftl::handle::OwnedHandle;
use ftl::interrupt::Interrupt;
use ftl::pci::PciEntry;
use ftl::prelude::*;
use ftl::println;
use ftl::rc::Rc;

use crate::virtio::ChainEntry;
use crate::virtio::Error as VirtioError;
use crate::virtio::VirtQueue;
use crate::virtio::VirtioPci;

mod virtio;

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

struct RxRequest {
    vaddr: usize,
    paddr: usize,
}

struct TxBuffer {
    vaddr: usize,
    paddr: usize,
    size: usize,
}

struct PendingWrite {
    completer: WriteCompleter,
    data: Vec<u8>,
}

struct Main {
    virtio: VirtioPci,
    rxq: VirtQueue,
    txq: VirtQueue,
    pending_rxs: Vec<Option<RxRequest>>,
    pending_txs: Vec<Option<TxBuffer>>,
    free_txs: Vec<TxBuffer>,
    pending_reads: VecDeque<ReadCompleter>,
    pending_writes: VecDeque<PendingWrite>,
    rx_queue: VecDeque<Vec<u8>>,
    net_ch_id: HandleId,
}

impl Application for Main {
    fn init(ctx: &mut Context) -> Self {
        println!("[virtio_net] starting...");

        // Look up virtio-net PCI device
        let mut entries: MaybeUninit<[PciEntry; 10]> = MaybeUninit::uninit();
        let n =
            ftl::pci::sys_pci_lookup(entries.as_mut_ptr() as *mut PciEntry, 10, 0x1af4, 0x1000)
                .unwrap();

        let devices =
            unsafe { core::slice::from_raw_parts(entries.as_ptr() as *const PciEntry, n) };
        println!("[virtio_net] found {} virtio-net PCI devices", n);

        assert!(n > 0, "no virtio-net device found");

        let entry = devices[0];
        println!(
            "[virtio_net] using PCI device at {:x}:{:x}",
            entry.bus, entry.slot
        );

        // Enable bus mastering
        ftl::pci::sys_pci_set_busmaster(entry.bus, entry.slot, true).unwrap();

        // Get BAR0 (I/O port base for legacy virtio)
        let bar0 = ftl::pci::sys_pci_get_bar(entry.bus, entry.slot, 0).unwrap();
        let iobase = (bar0 & 0xfffffffc) as u16;
        println!("[virtio_net] I/O base: {:#x}", iobase);

        // Get interrupt line and acquire it
        let irq = ftl::pci::sys_pci_get_interrupt_line(entry.bus, entry.slot).unwrap();
        println!("[virtio_net] IRQ: {}", irq);

        let interrupt = Interrupt::acquire(irq).unwrap();
        ctx.add_interrupt(Rc::new(interrupt)).unwrap();
        println!("[virtio_net] interrupt acquired");

        // Enable IOPL for direct I/O access
        ftl::syscall::sys_x64_iopl(true).unwrap();
        println!("[virtio_net] I/O port access enabled");

        // Initialize virtio device
        const VIRTIO_NET_F_MAC: u32 = 1 << 5;
        let virtio = VirtioPci::new(entry.bus, entry.slot, iobase);
        let guest_features = virtio.initialize1();
        assert!(
            guest_features & VIRTIO_NET_F_MAC != 0,
            "MAC feature not supported"
        );
        virtio.write_guest_features(guest_features);

        // Read MAC address
        let mut mac = [0u8; 6];
        for i in 0..6 {
            mac[i] = virtio.read_device_config8(i as u16);
        }
        println!(
            "[virtio_net] MAC address: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );

        let mut rxq = virtio.setup_virtqueue(0).unwrap();
        let mut txq = virtio.setup_virtqueue(1).unwrap();

        let ch_id = HandleId::from_raw(1);
        let ch = Channel::from_handle(OwnedHandle::from_raw(ch_id));
        ctx.add_channel(ch).unwrap();

        // Initialize pending_rxs with None for each possible descriptor
        let mut pending_rxs: Vec<Option<RxRequest>> = Vec::with_capacity(rxq.queue_size());
        for _ in 0..rxq.queue_size() {
            pending_rxs.push(None);
        }

        let mut pending_txs: Vec<Option<TxBuffer>> = Vec::with_capacity(txq.queue_size());
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
            pending_rxs[head.0 as usize] = Some(RxRequest { vaddr, paddr });
        }
        rxq.notify(&virtio);
        println!("[virtio_net] RX buffers prepared");

        // Complete virtio initialization.
        virtio.initialize2();
        println!("[virtio_net] virtio device initialized");

        Self {
            virtio,
            rxq,
            txq,
            pending_rxs,
            pending_txs,
            free_txs: Vec::new(),
            pending_reads: VecDeque::new(),
            pending_writes: VecDeque::new(),
            rx_queue: VecDeque::new(),
            net_ch_id: ch_id,
        }
    }

    fn irq(&mut self, _ctx: &mut Context, interrupt: &Rc<Interrupt>, _irq: u8) {
        let isr = self.virtio.read_isr();
        if isr & 1 != 0 {
            // Process received packets.
            let header_len = size_of::<VirtioNetHdr>();
            while let Some(used) = self.rxq.pop() {
                let Some(rx) = self.pending_rxs[used.head.0 as usize].take() else {
                    println!("missing a RX request for {:?}", used.head);
                    continue;
                };

                let total_len = used.total_len as usize;
                if total_len > header_len {
                    let payload_len =
                        min(total_len - header_len, RX_BUFFER_SIZE.saturating_sub(header_len));
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
                self.pending_rxs[head.0 as usize] = Some(rx);
            }

            self.rxq.notify(&self.virtio);

            self.flush_reads();
            self.flush_writes();
        }

        interrupt.acknowledge().unwrap();
    }

    fn read(&mut self, ctx: &mut Context, completer: ReadCompleter, _offset: usize, _len: usize) {
        if ctx.handle_id() != self.net_ch_id {
            completer.error(ErrorCode::InvalidArgument);
            return;
        }

        self.pending_reads.push_back(completer);
        self.flush_reads();
    }

    fn write(
        &mut self,
        ctx: &mut Context,
        completer: WriteCompleter,
        _offset: usize,
        len: usize,
    ) {
        if ctx.handle_id() != self.net_ch_id {
            completer.error(ErrorCode::InvalidArgument);
            return;
        }

        let mut data = vec![0u8; len];
        let read_len = match completer.read_data(0, &mut data) {
            Ok(len) => len,
            Err(error) => {
                println!("[virtio_net] failed to read tx data: {:?}", error);
                completer.error(error);
                return;
            }
        };
        if read_len == 0 {
            completer.complete(0);
            return;
        }
        data.truncate(read_len);
        self.pending_writes.push_back(PendingWrite { completer, data });
        self.flush_writes();
    }
}

impl Main {
    fn flush_reads(&mut self) {
        while !self.pending_reads.is_empty() && !self.rx_queue.is_empty() {
            let packet = self.rx_queue.pop_front().unwrap();
            let completer = self.pending_reads.pop_front().unwrap();
            let write_len = match completer.write_data(0, &packet) {
                Ok(len) => len,
                Err(error) => {
                    println!("[virtio_net] failed to write rx data: {:?}", error);
                    completer.error(error);
                    continue;
                }
            };
            completer.complete(write_len);
        }
    }

    fn flush_writes(&mut self) {
        self.drain_tx();
        while let Some(pending) = self.pending_writes.pop_front() {
            match self.send_packet(&pending.data) {
                Ok(()) => pending.completer.complete(pending.data.len()),
                Err(VirtioError::VirtQueueFull) => {
                    self.pending_writes.push_front(pending);
                    break;
                }
                Err(error) => {
                    println!("[virtio_net] failed to send tx: {:?}", error);
                    pending.completer.error(Self::tx_error_code(error));
                }
            }
        }
    }

    fn drain_tx(&mut self) {
        while let Some(used) = self.txq.pop() {
            if let Some(buf) = self.pending_txs[used.head.0 as usize].take() {
                self.free_txs.push(buf);
            }
        }
    }

    fn send_packet(&mut self, data: &[u8]) -> Result<(), VirtioError> {
        self.drain_tx();

        let header_len = size_of::<VirtioNetHdr>();
        let payload_len = data.len().max(MIN_ETH_FRAME);
        let total_len = header_len + payload_len;
        let mut buf = self.take_tx_buffer(total_len)?;

        unsafe {
            let hdr_ptr = buf.vaddr as *mut VirtioNetHdr;
            hdr_ptr.write(VirtioNetHdr {
                flags: 0,
                gso_type: 0,
                hdr_len: 0,
                gso_size: 0,
                csum_start: 0,
                csum_offset: 0,
            });

            let payload_ptr = (buf.vaddr + header_len) as *mut u8;
            core::ptr::write_bytes(payload_ptr, 0, payload_len);
            core::ptr::copy_nonoverlapping(data.as_ptr(), payload_ptr, data.len());
        }

        let head = match self.txq.push(&[
            ChainEntry::Read {
                paddr: buf.paddr as u64,
                len: header_len as u32,
            },
            ChainEntry::Read {
                paddr: (buf.paddr + header_len) as u64,
                len: payload_len as u32,
            },
        ]) {
            Ok(head) => head,
            Err(error) => {
                self.free_txs.push(buf);
                return Err(error);
            }
        };
        self.pending_txs[head.0 as usize] = Some(buf);
        self.txq.notify(&self.virtio);
        Ok(())
    }

    fn take_tx_buffer(&mut self, min_size: usize) -> Result<TxBuffer, VirtioError> {
        if let Some(index) = self.free_txs.iter().position(|buf| buf.size >= min_size) {
            return Ok(self.free_txs.swap_remove(index));
        }

        let size = TX_BUFFER_SIZE.max(min_size);
        let mut vaddr = 0usize;
        let mut paddr = 0usize;
        ftl::dmabuf::sys_dmabuf_alloc(size, &mut vaddr, &mut paddr)
            .map_err(VirtioError::DmaBufAlloc)?;
        Ok(TxBuffer { vaddr, paddr, size })
    }

    fn tx_error_code(error: VirtioError) -> ErrorCode {
        match error {
            VirtioError::DmaBufAlloc(code) => code,
            _ => ErrorCode::Unreachable,
        }
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
