#![no_std]
#![no_main]

use ftl_api::channel::Channel;
use ftl_api::collections::VecDeque;
use ftl_api::handle::OwnedHandle;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::types::idl::BytesField;
use ftl_api::types::message::MessageBuffer;
use ftl_api_autogen::apps::tcpip::Environ;
use ftl_api_autogen::apps::tcpip::Message;
use ftl_api_autogen::protocols::ethernet_device;
use smoltcp::iface::Config;
use smoltcp::iface::Interface;
use smoltcp::iface::SocketSet;
use smoltcp::phy::DeviceCapabilities;
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::EthernetAddress;
use smoltcp::wire::HardwareAddress;
use smoltcp::wire::IpAddress;
use smoltcp::wire::IpCidr;
use smoltcp::wire::IpListenEndpoint;

enum Context {
    Autopilot,
    Driver,
    Client,
}

struct RxTokenImpl(Vec<u8>);

impl smoltcp::phy::RxToken for RxTokenImpl {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        f(&mut self.0)
    }
}

struct TxTokenImpl<'a>(&'a mut DeviceImpl);

impl<'a> smoltcp::phy::TxToken for TxTokenImpl<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buf = [0u8; 1514];
        let ret = f(&mut buf[..len]);

        let tx = ethernet_device::Tx {
            payload: BytesField::new(buf, len.try_into().unwrap()),
        };
        if let Err(err) = self.0.driver_ch.send_with_buffer(&mut self.0.msgbuffer, tx) {
            warn!("failed to send: {:?}", err);
        }

        ret
    }
}

struct DeviceImpl {
    driver_ch: Channel,
    rx_queue: VecDeque<Vec<u8>>,
    msgbuffer: MessageBuffer,
}

impl DeviceImpl {
    pub fn new(driver_ch: Channel) -> DeviceImpl {
        DeviceImpl {
            driver_ch,
            rx_queue: VecDeque::new(),
            msgbuffer: MessageBuffer::new(),
        }
    }

    pub fn receive_pkt(&mut self, pkt: &[u8]) {
        self.rx_queue.push_back(pkt.to_vec());
    }
}

impl smoltcp::phy::Device for DeviceImpl {
    type RxToken<'a> = RxTokenImpl;
    type TxToken<'a> = TxTokenImpl<'a>;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = smoltcp::phy::Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps
    }

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.rx_queue
            .pop_front()
            .map(|pkt| (RxTokenImpl(pkt), TxTokenImpl(self)))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
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

    let driver_ch = env.depends.ethernet_device.take().unwrap();
    // FIXME: Clone using syscall
    let driver_ch_cloned = Channel::from_handle(OwnedHandle::from_raw(driver_ch.handle().id()));

    let mac = HardwareAddress::Ethernet(EthernetAddress([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]));
    let config = Config::new(mac.into());
    let mut device = DeviceImpl::new(driver_ch_cloned);
    let mut iface = Interface::new(config, &mut device, now());
    let mut sockets = SocketSet::new(Vec::with_capacity(16));

    let mut mainloop = Mainloop::<Context, Message>::new().unwrap();
    mainloop
        .add_channel(env.autopilot_ch.take().unwrap(), Context::Autopilot)
        .unwrap();
    mainloop.add_channel(driver_ch, Context::Driver).unwrap();

    iface.update_ip_addrs(|ip_addrs| {
        // FIXME:
        ip_addrs
            .push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24))
            .unwrap();
    });

    let rx_buf = tcp::SocketBuffer::new(vec![0; 8192]);
    let tx_buf = tcp::SocketBuffer::new(vec![0; 8192]);
    let mut sock = tcp::Socket::new(rx_buf, tx_buf);
    sock.listen(IpListenEndpoint {
        addr: None,
        port: 1234,
    })
    .unwrap();
    let sock_handle = sockets.add(sock);

    let mut buffer = MessageBuffer::new();
    loop {
        let ready = iface.poll(now(), &mut device, &mut sockets);

        match mainloop.next(&mut buffer) {
            Event::Message { ch, ctx, m } => {
                match (ctx, m) {
                    (Context::Autopilot, Message::NewclientRequest(m)) => {
                        info!("got new client: {:?}", m.handle());
                        let new_ch = Channel::from_handle(OwnedHandle::from_raw(m.handle()));
                        mainloop.add_channel(new_ch, Context::Client).unwrap();
                    }
                    // (Context::Client { counter }, _) => {
                    // }
                    (Context::Driver, Message::Rx(m)) => {
                        trace!("received {} bytes", m.payload().len());
                        device.receive_pkt(m.payload().as_slice());
                    }
                    _ => {
                        // TODO: dump message with fmt::Debug
                        panic!("unknown message");
                    }
                }
            }
            _ => {
                panic!("unexpected event");
            }
        }
    }
}
