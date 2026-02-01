#![no_std]
#![no_main]

use core::cmp::min;
use core::mem::MaybeUninit;
use core::mem::size_of;

use ftl::application::Application;
use ftl::application::Context;
use ftl::interrupt::Interrupt;
use ftl::pci::PciEntry;
use ftl::prelude::*;
use ftl::println;
use ftl::rc::Rc;

use crate::virtio::ChainEntry;
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

#[repr(C, packed)]
struct EthernetHeader {
    dst: [u8; 6],
    src: [u8; 6],
    ethertype: u16,
}

#[repr(C, packed)]
struct ArpPacket {
    htype: u16,
    ptype: u16,
    hlen: u8,
    plen: u8,
    oper: u16,
    sha: [u8; 6],
    spa: [u8; 4],
    tha: [u8; 6],
    tpa: [u8; 4],
}

const ETHERTYPE_ARP: u16 = 0x0806;
const ETHERTYPE_IPV4: u16 = 0x0800;
const ARP_HTYPE_ETHERNET: u16 = 1;
const ARP_OP_REQUEST: u16 = 1;
const ARP_OP_REPLY: u16 = 2;
const MIN_ETH_FRAME: usize = 60;
const RX_BUFFER_SIZE: usize = 1514 + size_of::<VirtioNetHdr>();

struct RxRequest {
    vaddr: usize,
    paddr: usize,
}

struct Main {
    virtio: VirtioPci,
    rxq: VirtQueue,
    txq: VirtQueue,
    mac: [u8; 6],
    pending_rxs: Vec<Option<RxRequest>>,
}

impl Application for Main {
    fn init(ctx: &mut Context) -> Self {
        println!("[virtio_net] starting...");

        // Look up virtio-net PCI device
        let mut entries: MaybeUninit<[PciEntry; 10]> = MaybeUninit::uninit();
        let n = ftl::pci::sys_pci_lookup(entries.as_mut_ptr() as *mut PciEntry, 10, 0x1af4, 0x1000)
            .unwrap();

        let devices = unsafe { entries.assume_init() };
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

        // Initialize pending_rxs with None for each possible descriptor
        let mut pending_rxs: Vec<Option<RxRequest>> = Vec::with_capacity(rxq.queue_size());
        for _ in 0..rxq.queue_size() {
            pending_rxs.push(None);
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

        println!("[virtio_net] sent ARP request");
        test_tx_packet(&virtio, &mut txq, mac);

        Self {
            virtio,
            rxq,
            txq,
            mac,
            pending_rxs,
        }
    }

    fn irq(&mut self, _ctx: &mut Context, interrupt: &Rc<Interrupt>, _irq: u8) {
        let isr = self.virtio.read_isr();
        if isr & 1 != 0 {
            // Process received packets.
            while let Some(used) = self.rxq.pop() {
                let Some(rx) = self.pending_rxs[used.head.0 as usize].take() else {
                    println!("missing a RX request for {:?}", used.head);
                    continue;
                };

                test_rx_packet(&rx, used.total_len);

                // Re-add the buffer to the RX queue.
                let chain = &[ChainEntry::Write {
                    paddr: rx.paddr as u64,
                    len: RX_BUFFER_SIZE as u32,
                }];
                let head = self.rxq.push(chain).unwrap();
                self.pending_rxs[head.0 as usize] = Some(rx);
            }

            self.rxq.notify(&self.virtio);
        }

        interrupt.acknowledge().unwrap();
    }
}

fn test_tx_packet(virtio: &VirtioPci, txq: &mut VirtQueue, mac: [u8; 6]) {
    // Send ARP request
    let sender_ip = [10, 0, 2, 15];
    let target_ip = [10, 0, 2, 2];

    let mut tx_vaddr = 0usize;
    let mut tx_paddr = 0usize;
    ftl::dmabuf::sys_dmabuf_alloc(4096, &mut tx_vaddr, &mut tx_paddr).unwrap();
    let packet_ptr = tx_vaddr as *mut u8;

    unsafe {
        let hdr_ptr = packet_ptr as *mut VirtioNetHdr;
        hdr_ptr.write(VirtioNetHdr {
            flags: 0,
            gso_type: 0,
            hdr_len: 0,
            gso_size: 0,
            csum_start: 0,
            csum_offset: 0,
        });

        let payload_ptr = packet_ptr.add(size_of::<VirtioNetHdr>());
        core::ptr::write_bytes(payload_ptr, 0, MIN_ETH_FRAME);

        let eth_ptr = payload_ptr as *mut EthernetHeader;
        eth_ptr.write(EthernetHeader {
            dst: [0xff; 6],
            src: mac,
            ethertype: u16::to_be(ETHERTYPE_ARP),
        });

        let arp_ptr = payload_ptr.add(size_of::<EthernetHeader>()) as *mut ArpPacket;
        arp_ptr.write(ArpPacket {
            htype: u16::to_be(ARP_HTYPE_ETHERNET),
            ptype: u16::to_be(ETHERTYPE_IPV4),
            hlen: 6,
            plen: 4,
            oper: u16::to_be(ARP_OP_REQUEST),
            sha: mac,
            spa: sender_ip,
            tha: [0; 6],
            tpa: target_ip,
        });
    }

    let header_len = size_of::<VirtioNetHdr>() as u32;
    let payload_len = MIN_ETH_FRAME as u32;
    let payload_paddr = tx_paddr + size_of::<VirtioNetHdr>();

    txq.push(&[
        ChainEntry::Read {
            paddr: tx_paddr as u64,
            len: header_len,
        },
        ChainEntry::Read {
            paddr: payload_paddr as u64,
            len: payload_len,
        },
    ])
    .unwrap();
    txq.notify(&virtio);
}

fn test_rx_packet(rx_req: &RxRequest, len: u32) {
    unsafe {
        let packet_ptr = rx_req.vaddr as *const u8;
        let payload_ptr = packet_ptr.add(size_of::<VirtioNetHdr>());
        let eth = &*(payload_ptr as *const EthernetHeader);
        let ethertype = u16::from_be(eth.ethertype);

        if ethertype == ETHERTYPE_ARP {
            let arp = &*(payload_ptr.add(size_of::<EthernetHeader>()) as *const ArpPacket);
            let oper = u16::from_be(arp.oper);

            if oper == ARP_OP_REPLY {
                println!("\x1b[1;32m[virtio_net] ARP reply received!\x1b[0m");
            } else {
                println!("[virtio_net] ARP packet, oper={}", oper);
            }
        } else {
            println!("[virtio_net] received ethertype={:#06x}", ethertype);
        }
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
