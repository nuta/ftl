#![no_std]
#![no_main]

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::OpenCompleter;
use ftl::application::ReadCompleter;
use ftl::application::WriteCompleter;
use ftl::channel::Channel;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::prelude::*;
use ftl::println;
use smoltcp::iface::Interface;
use smoltcp::iface::SocketHandle;
use smoltcp::iface::SocketSet;
use smoltcp::phy::DeviceCapabilities;
use smoltcp::socket::tcp;
use smoltcp::socket::tcp::ListenError;
use smoltcp::wire::EthernetAddress;
use smoltcp::wire::HardwareAddress;
use smoltcp::wire::IpCidr;
use smoltcp::wire::IpListenEndpoint;
use smoltcp::wire::Ipv4Address;
use smoltcp::wire::Ipv4Cidr;

enum Uri {
    TcpListen(IpListenEndpoint),
}

const TCP_BUFFER_SIZE: usize = 4096;

struct RxToken {}

impl smoltcp::phy::RxToken for RxToken {
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
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken;

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
        handle: SocketHandle,
        pending_reads: VecDeque<ReadCompleter>,
        pending_writes: VecDeque<WriteCompleter>,
    },
    TcpListener {
        handle: SocketHandle,
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
    pub fn tcp_listen(
        &mut self,
        ctx: &mut Context,
        endpoint: IpListenEndpoint,
    ) -> Result<Channel, ErrorCode> {
        let (our_ch, their_ch) = Channel::new()?;
        let rx_buf = tcp::SocketBuffer::new(vec![0; TCP_BUFFER_SIZE]);
        let tx_buf = tcp::SocketBuffer::new(vec![0; TCP_BUFFER_SIZE]);
        let mut socket = tcp::Socket::new(rx_buf, tx_buf);

        socket.set_nagle_enabled(false);
        socket.set_ack_delay(None);

        match socket.listen(endpoint) {
            Ok(_) => {}
            Err(ListenError::Unaddressable) => {
                return Err(ErrorCode::InvalidArgument);
            }
            Err(e) => {
                println!("unexpected listen error: {:?}", e);
                return Err(ErrorCode::Unreachable);
            }
        }

        let handle = self.sockets.add(socket);
        let handle_id = our_ch.handle().id();
        ctx.add_channel(our_ch)?;
        self.states.insert(
            handle_id,
            State::TcpListener {
                handle,
                pending_accepts: VecDeque::new(),
            },
        );

        Ok(their_ch)
    }
}

fn parse_uri(completer: &OpenCompleter) -> Result<Uri, ErrorCode> {
    let mut buf = [0; 256];
    let len = completer.read_uri(0, &mut buf)?;

    let Ok(uri) = core::str::from_utf8(&buf[..len]) else {
        return Err(ErrorCode::InvalidArgument);
    };

    // Split "tcp-listen:0.0.0.0:8080" into "tcp-listen" and "0.0.0.0:8080".
    let Some((scheme, rest)) = uri.split_once(':') else {
        return Err(ErrorCode::InvalidArgument);
    };

    match scheme {
        "tcp-listen" => {
            // Split "0.0.0.0:8080" into "0.0.0.0" and "8080".
            let Some((addr_str, port_str)) = rest.split_once(':') else {
                return Err(ErrorCode::InvalidArgument);
            };

            let Ok(addr) = addr_str.parse::<core::net::IpAddr>() else {
                return Err(ErrorCode::InvalidArgument);
            };

            let Ok(port) = port_str.parse::<u16>() else {
                return Err(ErrorCode::InvalidArgument);
            };

            let endpoint = if addr.is_unspecified() {
                IpListenEndpoint { addr: None, port }
            } else {
                // TODO: We don't support listening on specific addresses for now.
                return Err(ErrorCode::InvalidArgument);
            };
            Ok(Uri::TcpListen(endpoint))
        }
        _ => {
            // Unknown scheme.
            Err(ErrorCode::InvalidArgument)
        }
    }
}

impl Application for Main {
    fn init(ctx: &mut Context) -> Self {
        println!("TODO TODO TODO: fill in the MAC address");
        let hwaddr = [0u8; 6];
        let gw_ip = Ipv4Address::new(10, 0, 2, 1);
        let our_ip = IpCidr::Ipv4(Ipv4Cidr::new(Ipv4Address::new(10, 0, 2, 15), 24));

        let smol_clock = SmolClock::new();
        let hwaddr = HardwareAddress::Ethernet(EthernetAddress::from_bytes(&hwaddr));
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
        match parse_uri(&completer) {
            Ok(Uri::TcpListen(endpoint)) => {
                match self.tcp_listen(ctx, endpoint) {
                    Ok(new_ch) => completer.complete(new_ch),
                    Err(error) => completer.error(error),
                }
            }
            Err(error) => {
                println!("invalid URI: {:?}", error);
                completer.error(ErrorCode::InvalidArgument)
            }
        }
    }

    fn read(&mut self, ctx: &mut Context, completer: ReadCompleter, offset: usize, len: usize) {
        let Some(state) = self.states.get_mut(&ctx.handle_id()) else {
            println!("state not found for {:?}", ctx.handle_id());
            completer.error(ErrorCode::InvalidArgument);
            return;
        };

        match state {
            State::TcpConn {
                handle,
                pending_reads,
                ..
            } => {
                pending_reads.push_back(completer);
            }
            State::TcpListener {
                handle,
                pending_accepts,
            } => {
                completer.error(ErrorCode::Unsupported);
            }
        }
    }

    fn write(&mut self, ctx: &mut Context, completer: WriteCompleter, offset: usize, len: usize) {
        let Some(state) = self.states.get_mut(&ctx.handle_id()) else {
            println!("state not found for {:?}", ctx.handle_id());
            completer.error(ErrorCode::InvalidArgument);
            return;
        };

        match state {
            State::TcpConn {
                handle,
                pending_writes,
                ..
            } => {
                pending_writes.push_back(completer);
            }
            State::TcpListener {
                handle,
                pending_accepts,
            } => {
                completer.error(ErrorCode::Unsupported);
            }
        }
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
