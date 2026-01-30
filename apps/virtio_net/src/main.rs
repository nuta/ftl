#![no_std]
#![no_main]

use ftl::application::Context;
use ftl::application::OpenRequest;
use ftl::application::ReadRequest;
use ftl::application::WriteRequest;
use ftl::channel::Channel;
use ftl::dmabuf;
use ftl::dmabuf::DmaAddr;
use ftl::error::ErrorCode;
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
    pending_read: Option<ReadRequest>,
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
            pending_read: None,
        }
    }

    fn write(&mut self, ctx: &mut Context<Channel>, req: WriteRequest) {
        if self.txq.is_full(3) {
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

        self.txq
            .push(&[ChainEntry::Read {
                paddr: daddr.as_usize() as u64,
                len: written_len as u32,
            }])
            .unwrap();

        req.complete(written_len);
    }

    fn read(&mut self, ctx: &mut Context<Channel>, req: ReadRequest) {
        if let Some(chain) = self.rxq.pop() {
            // We have a pending RX packet. Reply immediately.
            let ChainEntry::Read { paddr, len } = chain.descs[0] else {
                println!("expected read-only descriptor");
                return;
            };

            let buf = self
                .dmabufs
                .get_by_daddr(DmaAddr::new(paddr as usize))
                .unwrap();
            let packet = &buf[size_of::<VirtioNetHdr>()..chain.total_len as usize];
            req.write_data(packet, 0).unwrap();
            req.complete(chain.total_len as usize);
        } else {
            // No pending RX packet. Complete later.
            self.pending_read = Some(req);
        }
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::main::<Main, Env>();
}
