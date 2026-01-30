#![no_std]
#![no_main]

use ftl::application::Context;
use ftl::application::OpenRequest;
use ftl::application::ReadRequest;
use ftl::application::WriteRequest;
use ftl::channel::Channel;
use ftl::collections::vec_deque::VecDeque;
use ftl::dmabuf;
use ftl::dmabuf::DmaAddr;
use ftl::error::ErrorCode;
use ftl::interrupt::Interrupt;
use ftl::println;

use crate::virtio::ChainEntry;

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

#[derive(serde::Deserialize)]
struct Env {
    pci_bus: u8,
    pci_slot: u8,
    iobase: u16,
}

struct Main {
    virtio: virtio::VirtioPci,
    rxq: virtio::VirtQueue,
    txq: virtio::VirtQueue,
    dmabufs: dmabuf::Pool,
    pending_reads: VecDeque<ReadRequest>,
}

impl ftl::application::Application<Env> for Main {
    fn init(env: Env) -> Self {
        println!("virtio_net: initializing");
        let virtio = virtio::VirtioPci::new(env.pci_bus, env.pci_slot, env.iobase);
        let rxq = virtio.setup_virtqueue(0).unwrap();
        let txq = virtio.setup_virtqueue(1).unwrap();
        Self {
            virtio,
            rxq,
            txq,
            dmabufs: dmabuf::Pool::new().unwrap(),
            pending_reads: VecDeque::new(),
        }
    }

    fn write(&mut self, ctx: &mut Context<Channel>, req: WriteRequest) {
        if self.txq.is_full(2) {
            req.error(ErrorCode::RetryLater);
            return;
        }

        let written_len = req.len;
        let (daddr, buf) = match self.dmabufs.alloc(written_len) {
            Ok(pair) => pair,
            Err(error) => {
                req.error(error);
                return;
            }
        };

        if let Err(error) = req.read_data(buf, 0) {
            req.error(ErrorCode::BadBuffer);
            return;
        }

        // TODO: Add virtio-net header
        let chain = &[ChainEntry::Read {
            paddr: daddr.as_usize() as u64,
            len: written_len as u32,
        }];
        self.txq.push(chain).unwrap();

        req.complete(written_len);
    }

    fn read(&mut self, ctx: &mut Context<Channel>, req: ReadRequest) {
        self.pending_reads.push_back(req);
        self.flush_rxq();
    }

    fn interrupt(&mut self, ctx: &mut Context<Interrupt>) {
        self.virtio.handle_interrupt();
        self.flush_rxq();
    }
}

impl Main {
    fn flush_rxq(&mut self) {
        while !self.rxq.is_empty() && !self.pending_reads.is_empty() {
            let chain = self.rxq.pop().unwrap();
            let req = self.pending_reads.pop_front().unwrap();

            let ChainEntry::Read { paddr, len } = chain.descs[0] else {
                println!("ignoring an unexpected descriptor");
                continue;
            };

            let daddr = DmaAddr::new(paddr as usize);
            let buf = self.dmabufs.get_by_daddr(daddr).unwrap();
            let packet = &buf[size_of::<VirtioNetHdr>()..chain.total_len as usize];
            req.write_data(packet, 0).unwrap();
            req.complete(chain.total_len as usize);
        }
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::main::<Main, Env>();
}
