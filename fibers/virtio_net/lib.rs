#![no_std]

use ftl_api::channel::Channel;
use ftl_api::environ::Environ;
use ftl_api::folio::Folio;
use ftl_api::handle::Handle;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::types::address::PAddr;
use ftl_api::types::message::Message;
use ftl_autogen::fibers::virtio_net::Deps;

const VIRTIO_NET_F_MAC: u64 = 1 << 5;
const QUEUE_RX: u16 = 0;
const QUEUE_TX: u16 = 1;

#[derive(Debug)]
enum State {
    Autopilot,
    Client,
}

struct VirtioNet {
    virtio: virtio::VirtioDevice,
}

impl VirtioNet {
    pub fn new(virtio: virtio::VirtioDevice) -> Self {
        VirtioNet { virtio }
    }

    pub fn send(&self, pkt: &[u8]) {
        todo!()
    }
}

pub fn main(mut env: Environ) {
    let deps: Deps = env.parse_deps().expect("failed to parse deps");

    let irq = env
        .device()
        .interrupts
        .as_ref()
        .unwrap()
        .get(0)
        .copied()
        .unwrap() as usize;

    println!("virtio_net: listening for irq {}", irq);
    let ret = deps
        .irq_controller
        .call(Message::ListenIrq { irq })
        .unwrap();
    println!("virtio_net: irq listener registered: {:?}", ret);

    let base_paddr = PAddr::new(env.device().reg as usize).unwrap();
    let mmio = Folio::map_paddr(base_paddr, 0x1000).unwrap();

    let transport = virtio::transports::mmio::VirtioMmio::new(mmio);
    let mut virtio = virtio::VirtioDevice::new(Box::new(transport));
    virtio.initialize(VIRTIO_NET_F_MAC, 2);
    let virtio_net = VirtioNet::new(virtio);

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
                virtio_net.send(&pkt);
            }
            (_state, _event) => {
                todo!();
            }
        }
    });
}
