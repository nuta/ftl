#![no_std]
#![no_main]

use core::marker::PhantomData;

use ftl_api::channel::Channel;
use ftl_api::handle::OwnedHandle;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::types::idl::StringField;
use ftl_api::types::message::MessageBuffer;
use ftl_api_autogen::apps::tcpip::Environ;
use ftl_api_autogen::apps::tcpip::Message;
use ftl_api_autogen::protocols::net_device::Tx;
use ftl_api_autogen::protocols::ping::PingReply;
use smoltcp::iface::Config;
use smoltcp::iface::Interface;
use smoltcp::iface::SocketSet;
use smoltcp::phy::Checksum;
use smoltcp::phy::ChecksumCapabilities;
use smoltcp::phy::DeviceCapabilities;
use smoltcp::time::Instant;
use smoltcp::wire::EthernetAddress;
use smoltcp::wire::HardwareAddress;

enum Context {
    Autopilot,
    Client { counter: i32 },
}

struct RxTokenImpl<'a>(&'a DeviceImpl);
impl<'a> smoltcp::phy::RxToken for RxTokenImpl<'a> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        todo!()
    }
}

struct TxTokenImpl<'a>(&'a DeviceImpl);
impl<'a> smoltcp::phy::TxToken for TxTokenImpl<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        debug_assert!(len <= self.0.buffer.data.len());

        f(&mut self.0.buffer.data[..len]);
        self.0.driver_ch.send();
    }
}

struct DeviceImpl {
    driver_ch: Channel,
    buffer: MessageBuffer,
}

impl DeviceImpl {
    pub fn new(driver_ch: Channel) -> DeviceImpl {
        DeviceImpl {
            driver_ch,
            buffer: MessageBuffer::new(),
        }
    }
}

impl smoltcp::phy::Device for DeviceImpl {
    type RxToken<'a> = RxTokenImpl<'a>;
    type TxToken<'a> = TxTokenImpl<'a>;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = smoltcp::phy::Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps
    }

    fn receive(&mut self, timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        todo!()
    }

    fn transmit(&mut self, timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(TxTokenImpl(self))
    }
}

fn now() -> Instant {
    // FIXME:
    Instant::from_millis(0)
}

#[ftl_api::main]
pub fn main(mut env: Environ) {
    info!("starting...");

    let mac = HardwareAddress::Ethernet(EthernetAddress([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]));
    let config = Config::new(mac.into());
    let mut device = DeviceImpl::new();
    let mut iface = Interface::new(config, &mut device, now());
    let mut sockets = SocketSet::new(Vec::with_capacity(16));

    let mut mainloop = Mainloop::<Context, Message>::new().unwrap();
    mainloop
        .add_channel(env.autopilot_ch.take().unwrap(), Context::Autopilot)
        .unwrap();

    let mut buffer = MessageBuffer::new();
    loop {
        let ready = iface.poll(now(), &mut device, &mut sockets);

        match mainloop.next(&mut buffer) {
            Event::Message { ch, ctx, m } => {
                match (ctx, m) {
                    (Context::Autopilot, Message::NewclientRequest(m)) => {
                        info!("got new client: {:?}", m.handle());
                        let new_ch = Channel::from_handle(OwnedHandle::from_raw(m.handle()));
                        mainloop
                            .add_channel(new_ch, Context::Client { counter: 0 })
                            .unwrap();
                    }
                    // (Context::Client { counter }, _) => {
                    // }
                    _ => {
                        // TODO: dump message with fmt::Debug
                        panic!("unknown message");
                    }
                }
            }
            _ => {
                panic!("unexpected event");
            }
            Event::Error(err) => {
                panic!("mainloop error: {:?}", err);
            }
        }
    }
}
