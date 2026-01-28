#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use core::ptr::read_volatile;
use core::ptr::write_volatile;
use core::sync::atomic::Ordering;
use core::sync::atomic::fence;

use ftl::pci::PciEntry;
use ftl::println;

// Virtio legacy I/O port offsets
const VIRTIO_PCI_HOST_FEATURES: u16 = 0;
const VIRTIO_PCI_GUEST_FEATURES: u16 = 4;
const VIRTIO_PCI_QUEUE_PFN: u16 = 8;
const VIRTIO_PCI_QUEUE_SIZE: u16 = 12;
const VIRTIO_PCI_QUEUE_SEL: u16 = 14;
const VIRTIO_PCI_QUEUE_NOTIFY: u16 = 16;
const VIRTIO_PCI_STATUS: u16 = 18;
const VIRTIO_PCI_ISR: u16 = 19;
const VIRTIO_PCI_CONFIG: u16 = 20; // MAC address starts here for net

// Device status bits
const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
const VIRTIO_STATUS_FEATURES_OK: u8 = 8;

// Virtio net header flags
const VIRTIO_NET_HDR_F_NONE: u8 = 0;
const VIRTIO_NET_HDR_GSO_NONE: u8 = 0;

// Virtqueue descriptor flags
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

// Virtio net feature bits
const VIRTIO_NET_F_MAC: u32 = 1 << 5;

const PAGE_SIZE: usize = 4096;
const TX_QUEUE: u16 = 1;

// I/O port access
#[inline(always)]
fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val) };
}

#[inline(always)]
fn outw(port: u16, val: u16) {
    unsafe { core::arch::asm!("out dx, ax", in("dx") port, in("ax") val) };
}

#[inline(always)]
fn outl(port: u16, val: u32) {
    unsafe { core::arch::asm!("out dx, eax", in("dx") port, in("eax") val) };
}

#[inline(always)]
fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") val) };
    val
}

#[inline(always)]
fn inw(port: u16) -> u16 {
    let val: u16;
    unsafe { core::arch::asm!("in ax, dx", in("dx") port, out("ax") val) };
    val
}

#[inline(always)]
fn inl(port: u16) -> u32 {
    let val: u32;
    unsafe { core::arch::asm!("in eax, dx", in("dx") port, out("eax") val) };
    val
}

/// Virtqueue descriptor (16 bytes)
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

/// Virtqueue available ring header
#[repr(C)]
struct VirtqAvail {
    flags: u16,
    idx: u16,
    // ring: [u16; queue_size] follows
}

/// Virtqueue used ring element
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

/// Virtqueue used ring header
#[repr(C)]
struct VirtqUsed {
    flags: u16,
    idx: u16,
    // ring: [VirtqUsedElem; queue_size] follows
}

/// Virtio network header (legacy, 10 bytes, but often 12 with padding)
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtioNetHdr {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
    // num_buffers: u16, // only in mergeable rx buffers
}

struct Virtqueue {
    desc_paddr: usize,
    avail_paddr: usize,
    used_paddr: usize,
    desc_vaddr: usize,
    avail_vaddr: usize,
    used_vaddr: usize,
    queue_size: u16,
    last_used_idx: u16,
    free_head: u16,
}

impl Virtqueue {
    /// Calculate the total size needed for a virtqueue
    fn calc_size(queue_size: u16) -> usize {
        let desc_size = 16 * queue_size as usize;
        let avail_size = 6 + 2 * queue_size as usize; // flags(2) + idx(2) + ring(2*n) + used_event(2)
        let used_size = 6 + 8 * queue_size as usize; // flags(2) + idx(2) + ring(8*n) + avail_event(2)

        // Align to page boundary between avail and used
        let avail_end = desc_size + avail_size;
        let used_offset = (avail_end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        used_offset + used_size
    }
}

fn virtqueue_setup(iobase: u16, queue_idx: u16) -> Option<Virtqueue> {
    unsafe {
        // Select queue
        outw(iobase + VIRTIO_PCI_QUEUE_SEL, queue_idx);

        // Read queue size
        let queue_size = inw(iobase + VIRTIO_PCI_QUEUE_SIZE);
        if queue_size == 0 {
            println!("Queue {} has size 0", queue_idx);
            return None;
        }
        println!("Queue {} size: {}", queue_idx, queue_size);

        // Calculate memory needed
        let total_size = Virtqueue::calc_size(queue_size);
        let alloc_size = (total_size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        // Allocate DMA buffer
        let mut vaddr = 0usize;
        let mut paddr = 0usize;
        ftl::dmabuf::sys_dmabuf_alloc(alloc_size, &mut vaddr, &mut paddr).ok()?;
        println!(
            "Allocated virtqueue: vaddr={:#x}, paddr={:#x}, size={}",
            vaddr, paddr, alloc_size
        );

        // Zero the memory
        core::ptr::write_bytes(vaddr as *mut u8, 0, alloc_size);

        // Calculate offsets
        let desc_size = 16 * queue_size as usize;
        let avail_size = 6 + 2 * queue_size as usize;
        let avail_end = desc_size + avail_size;
        let used_offset = (avail_end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        let desc_paddr = paddr;
        let avail_paddr = paddr + desc_size;
        let used_paddr = paddr + used_offset;

        let desc_vaddr = vaddr;
        let avail_vaddr = vaddr + desc_size;
        let used_vaddr = vaddr + used_offset;

        // Initialize descriptor chain (link all descriptors as free list)
        let descs = desc_vaddr as *mut VirtqDesc;
        for i in 0..(queue_size - 1) {
            (*descs.add(i as usize)).next = i + 1;
        }

        // Tell device the queue address (PFN = physical frame number)
        let pfn = (paddr / PAGE_SIZE) as u32;
        outl(iobase + VIRTIO_PCI_QUEUE_PFN, pfn);
        println!("Set queue PFN to {:#x}", pfn);

        Some(Virtqueue {
            desc_paddr,
            avail_paddr,
            used_paddr,
            desc_vaddr,
            avail_vaddr,
            used_vaddr,
            queue_size,
            last_used_idx: 0,
            free_head: 0,
        })
    }
}

fn send_packet(iobase: u16, vq: &mut Virtqueue, packet_data: &[u8]) {
    unsafe {
        // We need to allocate a DMA buffer for the packet
        let total_len = core::mem::size_of::<VirtioNetHdr>() + packet_data.len();
        let mut pkt_vaddr = 0usize;
        let mut pkt_paddr = 0usize;
        ftl::dmabuf::sys_dmabuf_alloc(total_len, &mut pkt_vaddr, &mut pkt_paddr).unwrap();

        // Write virtio net header
        let hdr = pkt_vaddr as *mut VirtioNetHdr;
        write_volatile(
            hdr,
            VirtioNetHdr {
                flags: VIRTIO_NET_HDR_F_NONE,
                gso_type: VIRTIO_NET_HDR_GSO_NONE,
                hdr_len: 0,
                gso_size: 0,
                csum_start: 0,
                csum_offset: 0,
            },
        );

        // Write packet data after header
        let data_ptr = (pkt_vaddr + core::mem::size_of::<VirtioNetHdr>()) as *mut u8;
        core::ptr::copy_nonoverlapping(packet_data.as_ptr(), data_ptr, packet_data.len());

        // Get a free descriptor
        let desc_idx = vq.free_head;
        let descs = vq.desc_vaddr as *mut VirtqDesc;
        let desc = descs.add(desc_idx as usize);

        // Update free head
        vq.free_head = read_volatile(&(*desc).next);

        // Set up descriptor for the entire buffer (header + data)
        write_volatile(
            desc,
            VirtqDesc {
                addr: pkt_paddr as u64,
                len: total_len as u32,
                flags: 0, // No NEXT, no WRITE (device reads this)
                next: 0,
            },
        );

        // Add to available ring
        let avail = vq.avail_vaddr as *mut VirtqAvail;
        let avail_idx = read_volatile(&(*avail).idx);
        let ring_ptr = (vq.avail_vaddr + 4) as *mut u16; // Skip flags and idx
        let ring_idx = (avail_idx % vq.queue_size) as usize;
        write_volatile(ring_ptr.add(ring_idx), desc_idx);

        // Memory barrier before updating idx (wmb - ensures descriptor writes are visible)
        fence(Ordering::Release);

        // Update available index
        write_volatile(&mut (*avail).idx, avail_idx.wrapping_add(1));

        // Memory barrier before notify (wmb - ensures idx update is visible)
        fence(Ordering::Release);

        // Notify the device
        outw(iobase + VIRTIO_PCI_QUEUE_NOTIFY, TX_QUEUE);

        println!(
            "Sent packet: desc_idx={}, avail_idx={}, len={}",
            desc_idx, avail_idx, total_len
        );
    }
}

fn build_arp_request(
    buf: &mut [u8],
    src_mac: [u8; 6],
    src_ip: [u8; 4],
    target_ip: [u8; 4],
) -> usize {
    // Ethernet header (14 bytes)
    // Destination MAC: broadcast
    buf[0..6].copy_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    // Source MAC
    buf[6..12].copy_from_slice(&src_mac);
    // EtherType: ARP (0x0806)
    buf[12] = 0x08;
    buf[13] = 0x06;

    // ARP header (28 bytes)
    // Hardware type: Ethernet (1)
    buf[14] = 0x00;
    buf[15] = 0x01;
    // Protocol type: IPv4 (0x0800)
    buf[16] = 0x08;
    buf[17] = 0x00;
    // Hardware address length: 6
    buf[18] = 6;
    // Protocol address length: 4
    buf[19] = 4;
    // Operation: request (1)
    buf[20] = 0x00;
    buf[21] = 0x01;
    // Sender hardware address
    buf[22..28].copy_from_slice(&src_mac);
    // Sender protocol address
    buf[28..32].copy_from_slice(&src_ip);
    // Target hardware address (unknown, set to 0)
    buf[32..38].copy_from_slice(&[0x00; 6]);
    // Target protocol address
    buf[38..42].copy_from_slice(&target_ip);

    42 // Total length: 14 (eth) + 28 (arp)
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
    println!("Found {} virtio-net PCI devices", n);

    if n == 0 {
        println!("No virtio-net device found!");
        loop {
            unsafe { core::arch::asm!("hlt") }
        }
    }

    let entry = devices[0];
    println!("Using device {:x}:{:x}", entry.bus, entry.slot);

    // Enable bus mastering
    ftl::pci::sys_pci_set_busmaster(entry.bus, entry.slot, true).unwrap();

    // Get BAR0 (I/O port base for legacy virtio)
    let bar0 = ftl::pci::sys_pci_get_bar(entry.bus, entry.slot, 0).unwrap();
    let iobase = (bar0 & 0xfffffffc) as u16; // Mask off the I/O space indicator bit
    println!("I/O base: {:#x}", iobase);

    // Enable IOPL for direct I/O access
    ftl::syscall::sys_x64_iopl(true).unwrap();
    println!("IOPL enabled");

    unsafe {
        // Reset device
        outb(iobase + VIRTIO_PCI_STATUS, 0);
        println!("Device reset");

        // Acknowledge device
        outb(iobase + VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);
        println!("Device acknowledged");

        // Tell device we're a driver
        outb(
            iobase + VIRTIO_PCI_STATUS,
            VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER,
        );
        println!("Driver bit set");

        // Read device features
        let features = inl(iobase + VIRTIO_PCI_HOST_FEATURES);
        println!("Device features: {:#x}", features);

        // Accept features (we'll just accept MAC feature for now)
        let guest_features = features & VIRTIO_NET_F_MAC;
        outl(iobase + VIRTIO_PCI_GUEST_FEATURES, guest_features);
        println!("Guest features: {:#x}", guest_features);

        // Read MAC address (at config offset for net device)
        let mut mac = [0u8; 6];
        for i in 0..6 {
            mac[i] = inb(iobase + VIRTIO_PCI_CONFIG + i as u16);
        }
        println!(
            "MAC address: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );

        // Set up TX queue (queue 1)
        let mut tx_vq = virtqueue_setup(iobase, TX_QUEUE).expect("Failed to set up TX queue");

        // Mark driver ready
        outb(
            iobase + VIRTIO_PCI_STATUS,
            VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
        );
        println!("Device ready (DRIVER_OK set)");

        // Build and send an ARP request
        let mut packet = [0u8; 64]; // Minimum ethernet frame size
        let src_ip = [10, 0, 2, 15]; // Typical QEMU user network guest IP
        let target_ip = [10, 0, 2, 2]; // Typical QEMU user network gateway
        let len = build_arp_request(&mut packet, mac, src_ip, target_ip);

        println!("Sending ARP request...");
        send_packet(iobase, &mut tx_vq, &packet[..len]);
        println!("ARP request sent!");

        // Wait a bit and check if packet was consumed
        for _ in 0..100000 {
            core::arch::asm!("pause");
        }

        let used = tx_vq.used_vaddr as *const VirtqUsed;
        let used_idx = read_volatile(&(*used).idx);
        println!(
            "TX used idx: {}, last_used: {}",
            used_idx, tx_vq.last_used_idx
        );

        if used_idx != tx_vq.last_used_idx {
            println!("Packet was consumed by device!");
        }
    }

    println!("Done!");
    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}
