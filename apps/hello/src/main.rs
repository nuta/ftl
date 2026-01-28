#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use core::mem::size_of;

use ftl::pci::PciEntry;
use ftl::println;

use crate::virtio::ChainEntry;
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

#[unsafe(no_mangle)]
fn main() {
    println!(
        "\x1b[1m\x1b[32mHello\x1b[0m\x1b[1m \x1b[1m\x1b[33mworld\x1b[0m\x1b[1m \x1b[1m\x1b[36mfrom\x1b[0m\x1b[1m \x1b[1m\x1b[35msystem call!\x1b[0m\x1b[1m\x1b[0m"
    );

    let mut entries: MaybeUninit<[PciEntry; 10]> = MaybeUninit::uninit();
    let n = ftl::pci::sys_pci_lookup(entries.as_mut_ptr() as *mut PciEntry, 10, 0x1af4, 0x1000)
        .unwrap();

    let devices = unsafe { entries.assume_init() };
    println!("[virtio_net] found {} virtio-net PCI devices", n);

    debug_assert!(n > 0, "no virtio-net device found");
    debug_assert!(n <= 1, "multiple virtio-net devices found");

    let entry = devices[0];
    println!(
        "[virtio_net] using PCI device at {:x}:{:x}",
        entry.bus, entry.slot
    );

    // Enable bus mastering
    ftl::pci::sys_pci_set_busmaster(entry.bus, entry.slot, true).unwrap();

    // Get BAR0 (I/O port base for legacy virtio)
    let bar0 = ftl::pci::sys_pci_get_bar(entry.bus, entry.slot, 0).unwrap();
    let iobase = (bar0 & 0xfffffffc) as u16; // Mask off the I/O space indicator bit
    println!("[virtio_net] I/O base: {:#x}", iobase);

    // Enable IOPL for direct I/O access
    ftl::syscall::sys_x64_iopl(true).unwrap();
    println!("[virtio_net] I/O port access enabled");

    const VIRTIO_NET_F_MAC: u32 = 1 << 5;
    let virtio = VirtioPci::new(entry.bus, entry.slot, iobase);
    let guest_features = virtio.initialize1();
    assert!(
        guest_features & VIRTIO_NET_F_MAC != 0,
        "MAC feature not supported"
    );
    virtio.write_guest_features(guest_features);

    let mut mac = [0u8; 6];
    for i in 0..6 {
        mac[i] = virtio.read_device_config8(i as u16);
    }
    println!(
        "[virtio_net] MAC address: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );

    let mut txq = virtio.setup_virtqueue(1).unwrap();
    virtio.initialize2();

    const ETHERTYPE_ARP: u16 = 0x0806;
    const ETHERTYPE_IPV4: u16 = 0x0800;
    const ARP_HTYPE_ETHERNET: u16 = 1;
    const ARP_OP_REQUEST: u16 = 1;
    const MIN_ETH_FRAME: usize = 60;

    let sender_ip = [10, 0, 2, 15];
    let target_ip = [10, 0, 2, 2];

    let mut packet_vaddr = 0usize;
    let mut packet_paddr = 0usize;
    ftl::dmabuf::sys_dmabuf_alloc(4096, &mut packet_vaddr, &mut packet_paddr).unwrap();
    let packet_ptr = packet_vaddr as *mut u8;

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
    let payload_paddr = packet_paddr + size_of::<VirtioNetHdr>();
    txq.push(&[
        ChainEntry::Read {
            paddr: packet_paddr as u64,
            len: header_len,
        },
        ChainEntry::Read {
            paddr: payload_paddr as u64,
            len: payload_len,
        },
    ])
    .unwrap();
    txq.notify(&virtio);

    println!("[virtio_net] sent an ARP request packet");
    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}
