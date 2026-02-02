#![no_std]
#![no_main]

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::OpenCompleter;
use ftl::application::ReadCompleter;
use ftl::application::WriteCompleter;
use ftl::collections::VecDeque;
use ftl::handle::HandleId;
use ftl::prelude::*;
use ftl::println;
use smoltcp::iface::Interface;
use smoltcp::iface::SocketHandle;
use smoltcp::iface::SocketSet;
use smoltcp::phy::DeviceCapabilities;
use smoltcp::socket::tcp;

struct RxToken<'a> {}

impl<'a> smoltcp::phy::RxToken<'a> for RxToken<'a> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        todo!()
    }
}

struct TxToken {}

impl smoltcp::phy::TxToken for TxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        todo!()
    }
}

struct Device {}

impl Device {
    fn new() -> Self {
        Self {}
    }
}

impl smoltcp::phy::Device for Device {
    type RxToken<'a> = RxToken<'a>;
    type TxToken = TxToken;

    fn receive(
        &mut self,
        timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        todo!()
    }

    fn transmit(&mut self, timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        todo!()
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = smoltcp::phy::Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps
    }
}

enum State {
    TcpConn {
        socket: SocketHandle,
        pending_reads: VecDeque<ReadCompleter>,
        pending_writes: VecDeque<WriteCompleter>,
    },
    TcpListener {
        socket: SocketHandle,
        pending_accepts: VecDeque<OpenCompleter>,
    },
}

struct SmolClock {}

impl SmolClock {
    fn new() -> Self {
        Self {}
    }

    fn now(&self) -> smoltcp::time::Instant {
        // TODO:
        smoltcp::time::Instant::from_secs(0)
    }
}

struct Main {
    sockets: SocketSet<'static>,
    states: HashMap<HandleId, State>,
    device: Device,
    iface: Interface,
}

impl Main {
    pub fn tcp_listen(&mut self) -> SocketHandle {
        let rx_buf = tcp::SocketBuffer::new(vec![0; TCP_BUFFER_SIZE]);
        let tx_buf = tcp::SocketBuffer::new(vec![0; TCP_BUFFER_SIZE]);
        let mut socket = tcp::Socket::new(rx_buf, tx_buf);
        let socket = self.sockets.add(socket);
        self.states.insert(
            socket,
            State::TcpListener {
                socket,
                pending_accepts: VecDeque::new(),
            },
        );

        socket
    }
}

impl Application for Main {
    fn init(ctx: &mut Context) -> Self {
        println!("TODO TODO TODO: fill in the MAC address");
        let hwaddr = [0u8; 6];
        let gw_ip = Ipv4Address::new(10, 0, 2, 1);

        let smol_clock = SmolClock::new();
        let hwaddr = HardwareAddress::Ethernet(EthernetAddress::from_bytes(hwaddr));
        let config = smoltcp::iface::Config::new(hwaddr);
        let mut device = Device::new();
        let mut iface = Interface::new(config, &mut device, smol_clock.now());

        iface.routes_mut().add_default_ipv4_route(gw_ip).unwrap();
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs.push(our_ip).unwrap();
        });

        Self {
            sockets: SocketSet::new(Vec::new()),
            states: HashMap::new(),
            device: Device::new(),
            iface: iface,
        }
    }

    fn open(&mut self, ctx: &mut Context, completer: OpenCompleter) {
        println!("received an unexpected message: open");
        completer.error(ErrorCode::Unsupported)
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
