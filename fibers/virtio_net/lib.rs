#![no_std]
#![feature(offset_of)]

use core::mem::offset_of;
use core::mem::size_of;

use ftl_api::channel::Channel;
use ftl_api::environ::Environ;
use ftl_api::folio::Folio;
use ftl_api::handle::Handle;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::print;
use ftl_api::types::address::PAddr;
use ftl_api::types::address::VAddr;
use ftl_api::types::environ::Device;
use ftl_api::types::message::Message;
use ftl_autogen::fibers::virtio_net::Deps;
use packetbuf::zerocopy::EtherType;
use packetbuf::zerocopy::EthernetHeader;
use packetbuf::zerocopy::MacAddr;
use packetbuf::PacketBuf;
use virtio::transports::VirtioTransport;
use virtio::virtqueue::VirtQueue;
use virtio::virtqueue::VirtqDescBuffer;
use virtio::virtqueue::VirtqUsedChain;
use virtio::VIRTIO_DEVICE_TYPE_NET;

const VIRTIO_NET_F_MAC: u64 = 1 << 5;
const QUEUE_RX: usize = 0;
const QUEUE_TX: usize = 1;

#[derive(Debug)]
enum State {
    Autopilot,
    Client,
}

pub fn new_virtio(irq_controller: &Channel, devices: &[Device]) -> Box<dyn VirtioTransport> {
    for device in devices {
        let base_paddr = PAddr::new(device.reg as usize).unwrap();
        let mmio = Folio::map_paddr(base_paddr, 0x1000).unwrap();

        let mut transport = virtio::transports::mmio::VirtioMmio::new(mmio);
        match transport.probe() {
            Some(device_type) if device_type == VIRTIO_DEVICE_TYPE_NET => {
                println!("virtio_net: found virtio_net device");
            }
            _ => {
                continue;
            }
        }

        let irq = device.interrupts.as_ref().unwrap().get(0).copied().unwrap() as usize;

        println!("virtio_net: listening for irq {}", irq);
        let ret = irq_controller.call(Message::ListenIrq { irq }).unwrap();
        println!("virtio_net: irq listener registered: {:?}", ret);

        let mut transport = Box::new(transport) as Box<dyn VirtioTransport>;
        transport.initialize(VIRTIO_NET_F_MAC, 2).unwrap();
        return transport;
    }

    panic!("virtio_net: no virtio_net device found");
}

const PACKET_LEN_MAX: usize = 2048;

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
struct VirtioNetHeader {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    checksum_start: u16,
    checksum_offset: u16,
}

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
struct VirtioNetConfig {
    mac: [u8; 6],
}

#[derive(Copy, Clone)]
pub struct BufferIndex(usize);

pub struct BufferPool {
    folio: Folio,
    free_indices: Vec<BufferIndex>,
    buffer_size: usize,
    num_buffers: usize,
}

impl BufferPool {
    pub fn new(buffer_size: usize, num_buffers: usize) -> BufferPool {
        let buffer = Folio::alloc(num_buffers).unwrap();
        let mut free_indices = Vec::new();
        for i in 0..num_buffers {
            free_indices.push(BufferIndex(i));
        }

        BufferPool {
            folio: buffer,
            free_indices,
            buffer_size,
            num_buffers,
        }
    }

    pub fn pop_free(&mut self) -> Option<BufferIndex> {
        self.free_indices.pop()
    }

    pub fn push_free(&mut self, index: BufferIndex) {
        self.free_indices.push(index);
    }

    pub fn paddr_to_index(&self, paddr: PAddr) -> Option<BufferIndex> {
        debug_assert!(
            paddr.as_usize() % self.buffer_size == 0,
            "paddr is not aligned"
        );

        // TODO: paddr may not be in the same folio
        let offset = paddr.as_usize() - self.folio.paddr().as_usize();
        let index = offset / self.buffer_size;
        if index < self.num_buffers {
            Some(BufferIndex(index))
        } else {
            None
        }
    }

    pub fn vaddr(&self, index: BufferIndex) -> VAddr {
        debug_assert!(index.0 < self.num_buffers);
        self.folio.vaddr().add(index.0 * self.buffer_size)
    }

    pub fn paddr(&self, index: BufferIndex) -> PAddr {
        debug_assert!(index.0 < self.num_buffers);
        self.folio.paddr().add(index.0 * self.buffer_size)
    }
}

pub struct VirtioNet {
    mac: MacAddr,
    virtio_transport: Box<dyn VirtioTransport>,
    tx_virtq: VirtQueue,
    rx_virtq: VirtQueue,
    tx_buffers: BufferPool,
    rx_buffers: BufferPool,
}

impl VirtioNet {
    pub fn initialize(irq_controller: &Channel, devices: &[Device]) -> VirtioNet {
        let mut virtio_transport = new_virtio(irq_controller, devices);
        let mut virtqueues = virtio_transport.initialize(VIRTIO_NET_F_MAC, 2).unwrap();
        let mut tx_virtq = virtqueues.get_mut(QUEUE_TX).unwrap().take().unwrap();
        let mut rx_virtq = virtqueues.get_mut(QUEUE_RX).unwrap().take().unwrap();

        // Read the MAC address.
        let mut raw_mac = [0; 6];
        for (i, byte) in raw_mac.iter_mut().enumerate() {
            *byte =
                virtio_transport.read_device_config8((offset_of!(VirtioNetConfig, mac) + i) as u16);
        }
        let mac = MacAddr::from_bytes(raw_mac);

        println!("virtio-net: MAC address is {}", mac);

        let tx_ring_len = tx_virtq.num_descs() as usize;
        let rx_ring_len = rx_virtq.num_descs() as usize;
        let mut tx_buffers = BufferPool::new(PACKET_LEN_MAX, tx_ring_len);
        let mut rx_buffers = BufferPool::new(PACKET_LEN_MAX, rx_ring_len);

        while let Some(index) = rx_buffers.pop_free() {
            rx_virtq.enqueue(&[VirtqDescBuffer::WritableFromDevice {
                paddr: rx_buffers.paddr(index),
                len: PACKET_LEN_MAX,
            }])
        }

        VirtioNet {
            mac,
            virtio_transport,
            tx_virtq,
            rx_virtq,
            tx_buffers,
            rx_buffers,
        }
    }

    pub fn mac(&self) -> MacAddr {
        self.mac
    }

    pub fn transmit(&mut self, dst: MacAddr, ether_type: EtherType, mut buf: PacketBuf) {
        let mut header = buf.prepend::<EthernetHeader>().unwrap();
        header.dst = dst.as_bytes();
        header.src = self.mac.as_bytes();
        header.ethertype.set(ether_type as u16);
        self.do_transmit(buf.as_bytes());
    }

    // FIXME: frame might be DMA-transferred to the device after this function returns.
    fn do_transmit(&mut self, frame: &[u8]) {
        let buffer_index = self.tx_buffers.pop_free().expect("no free tx buffers");
        let vaddr = self.tx_buffers.vaddr(buffer_index);
        let paddr = self.tx_buffers.paddr(buffer_index);

        println!(
            "virtio-net: transmitting {} octets (paddr={})",
            frame.len(),
            paddr,
        );

        // Fill the virtio-net header.
        let header_len = size_of::<VirtioNetHeader>();
        assert!(frame.len() <= PACKET_LEN_MAX - header_len);
        let header = unsafe { &mut *vaddr.as_mut_ptr::<VirtioNetHeader>() };
        header.flags = 0;
        header.gso_type = 0;
        header.gso_size = 0;
        header.checksum_start = 0;
        header.checksum_offset = 0;

        // Copy the payload into the our buffer.
        let payload_addr = unsafe { vaddr.as_mut_ptr::<u8>().add(header_len) };
        unsafe {
            payload_addr.copy_from_nonoverlapping(frame.as_ptr(), frame.len());
        }

        // Construct a descriptor chain.
        let chain = &[
            VirtqDescBuffer::ReadOnlyFromDevice {
                paddr,
                len: header_len,
            },
            VirtqDescBuffer::ReadOnlyFromDevice {
                paddr: paddr.add(header_len),
                len: frame.len(),
            },
        ];

        // Enqueue the transmission request and kick the device.
        self.tx_virtq.enqueue(chain);
        self.tx_virtq.notify(&mut *self.virtio_transport);
    }

    pub fn handle_interrupt(&mut self) {
        println!("virtio-net: interrupt");
        // TODO: check ISR status

        while let Some(VirtqUsedChain { descs, total_len }) = self.rx_virtq.pop_used() {
            debug_assert!(descs.len() == 1);
            let paddr = match descs[0] {
                VirtqDescBuffer::WritableFromDevice { paddr, .. } => paddr,
                VirtqDescBuffer::ReadOnlyFromDevice { .. } => unreachable!(),
            };

            let buffer_index = self.rx_buffers.paddr_to_index(paddr).unwrap();
            let vaddr = self.rx_buffers.vaddr(buffer_index);

            let buffer = {
                if total_len < size_of::<VirtioNetHeader>() {
                    println!("virtio-net: received a too short buffer, ignoring...");
                    continue;
                }

                println!(
                    "virtio-net: received {} octets (paddr={}, payload_len={})",
                    total_len,
                    paddr,
                    total_len - size_of::<VirtioNetHeader>(),
                );

                unsafe {
                    core::slice::from_raw_parts(
                        vaddr.as_ptr::<u8>().add(size_of::<VirtioNetHeader>()),
                        total_len - size_of::<VirtioNetHeader>(),
                    )
                }
            };

            // hexdump of buffer
            for (i, byte) in buffer.iter().enumerate() {
                print!("{:02x} ", byte);
                if i % 16 == 15 {
                    println!();
                }
            }

            self.rx_virtq
                .enqueue(&[VirtqDescBuffer::WritableFromDevice {
                    paddr,
                    len: PACKET_LEN_MAX,
                }]);
        }
    }
}

pub fn main(mut env: Environ) {
    let deps: Deps = env.parse_deps().expect("failed to parse deps");
    let virtio_net =
        VirtioNet::initialize(&deps.irq_controller, env.devices().expect("no devices"));

    let mut mainloop = Mainloop::new();
    mainloop
        .add_channel(deps.autopilot, State::Autopilot)
        .unwrap();

    mainloop.run(move |changes, state, event| {
        match (state, event) {
            (State::Autopilot, Event::Message(_, Message::NewClient { ch: handle })) => {
                let ch = Channel::from_handle(Handle::new(handle));
                changes.add_channel(ch, State::Client);
            }
            (State::Client, Event::Message(_, Message::NetworkTx(pkt))) => {
                // virtio_net.transmit(&pkt);
                todo!();
            }
            (_state, _event) => {
                todo!();
            }
        }
    });
}
