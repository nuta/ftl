#![no_std]
#![no_main]
#![allow(unused)]

use core::cell::RefCell;

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::OpenCompleter;
use ftl::application::ReadCompleter;
use ftl::application::WriteCompleter;
use ftl::channel::Buffer;
use ftl::channel::BufferMut;
use ftl::channel::Channel;
use ftl::channel::Message;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::handle::OwnedHandle;
use ftl::prelude::*;
use ftl::println;
use ftl::rc::Rc;
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
const NET_RX_BUFFER_SIZE: usize = 1514;
const RX_QUEUE_SIZE: usize = 1;

struct RxToken {
    buffer: Vec<u8>,
}

impl smoltcp::phy::RxToken for RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.buffer)
    }
}

struct TxToken<'a> {
    ch: &'a Channel,
}

impl smoltcp::phy::TxToken for TxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buf = vec![0u8; len];
        let result = f(&mut buf);

        let msg = Message::Write {
            offset: 0,
            data: Buffer::Vec(buf),
        };

        if let Err(error) = self.ch.send(msg) {
            // TODO: Add a semaphore to limit the number of inflight writes.
            println!("failed to send packet: {:?}", error);
        }

        result
    }
}

struct Device {
    ch: Rc<Channel>,
    rx_queue: VecDeque<Vec<u8>>,
    inflight_reads: usize,
}

impl Device {
    fn new() -> Self {
        let ch_id = HandleId::from_raw(1);
        let ch = Rc::new(Channel::from_handle(OwnedHandle::from_raw(ch_id)));
        Self {
            ch,
            rx_queue: VecDeque::new(),
            inflight_reads: 0,
        }
    }

    fn channel(&self) -> Rc<Channel> {
        self.ch.clone()
    }

    fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    fn on_read_reply(&mut self, buf: BufferMut, len: usize) {
        debug_assert!(self.inflight_reads > 0);
        self.inflight_reads -= 1;

        if len == 0 {
            self.fill_rx();
            return;
        }

        let packet = match buf {
            BufferMut::Vec(mut data) => {
                let len = len.min(data.len());
                data.truncate(len);
                data
            }
            _ => unreachable!(),
        };

        self.rx_queue.push_back(packet);
        self.fill_rx();
    }

    fn fill_rx(&mut self) {
        while self.rx_queue.len() + self.inflight_reads < RX_QUEUE_SIZE {
            let msg = Message::Read {
                offset: 0,
                data: BufferMut::Vec(vec![0u8; NET_RX_BUFFER_SIZE]),
            };

            match self.ch.send(msg) {
                Ok(()) => {
                    self.inflight_reads += 1;
                }
                Err(error) => {
                    println!("failed to send a packet to drivers: {:?}", error);
                    break;
                }
            }
        }
    }
}

impl smoltcp::phy::Device for Device {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken<'a>;

    fn receive(
        &mut self,
        timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(packet) = self.rx_queue.pop_front() {
            let rx = RxToken { buffer: packet };
            let tx = TxToken {
                ch: self.ch.as_ref(),
            };
            Some((rx, tx))
        } else {
            if self.inflight_reads == 0 {
                self.fill_rx();
            }
            None
        }
    }

    fn transmit(&mut self, timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        Some(TxToken {
            ch: self.ch.as_ref(),
        })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = smoltcp::phy::Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps
    }
}

enum State {
    Driver,
    Control,
    TcpConn {
        handle: SocketHandle,
        pending_reads: VecDeque<ReadCompleter>,
        pending_writes: VecDeque<WriteCompleter>,
        channel_id: HandleId,
        channel_closed: bool,
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
    smol_clock: SmolClock,
    sockets: SocketSet<'static>,
    states_by_ch: HashMap<HandleId, Rc<RefCell<State>>>,
    states_by_handle: HashMap<SocketHandle, Rc<RefCell<State>>>,
    device: Device,
    iface: Interface,
}

impl Main {
    fn do_tcp_listen(&mut self, endpoint: IpListenEndpoint) -> Result<SocketHandle, ErrorCode> {
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
        Ok(handle)
    }

    pub fn tcp_listen(
        &mut self,
        ctx: &mut Context,
        endpoint: IpListenEndpoint,
    ) -> Result<Channel, ErrorCode> {
        let (our_ch, their_ch) = Channel::new()?;
        let handle = self.do_tcp_listen(endpoint)?;
        let ch_id = our_ch.handle().id();
        ctx.add_channel(our_ch)?;

        let state = Rc::new(RefCell::new(State::TcpListener {
            handle,
            pending_accepts: VecDeque::new(),
        }));

        self.states_by_ch.insert(ch_id, state.clone());
        self.states_by_handle.insert(handle, state);
        Ok(their_ch)
    }

    pub fn tcp_accept(
        &mut self,
        ctx: &mut Context,
        accepted_handle: SocketHandle,
        endpoint: IpListenEndpoint,
    ) -> Result<Channel, ErrorCode> {
        let (our_ch, their_ch) = Channel::new()?;
        let new_ch_id = our_ch.handle().id();
        ctx.add_channel(our_ch)?;

        // Create a new listen socket.
        let new_listen_handle = self.do_tcp_listen(endpoint)?;

        let conn_state = Rc::new(RefCell::new(State::TcpConn {
            handle: accepted_handle,
            pending_reads: VecDeque::new(),
            pending_writes: VecDeque::new(),
            channel_id: new_ch_id,
            channel_closed: false,
        }));

        // Replace the accepted socket's state.
        self.states_by_ch.insert(new_ch_id, conn_state.clone());
        let listen_state = self
            .states_by_handle
            .insert(accepted_handle, conn_state)
            .unwrap();
        self.states_by_handle
            .insert(new_listen_handle, listen_state);

        Ok(their_ch)
    }

    pub fn poll(&mut self, ctx: &mut Context) {
        use smoltcp::socket::Socket;

        let now = self.smol_clock.now();
        let result = self.iface.poll(now, &mut self.device, &mut self.sockets);
        let mut accepted_sockets = Vec::new();
        let mut destroyed_sockets = Vec::new();
        for (handle, socket) in self.sockets.iter_mut() {
            let state = self.states_by_handle.get(&handle).unwrap();
            match socket {
                Socket::Tcp(socket) => {
                    let mut state_borrow = state.borrow_mut();
                    match socket.state() {
                        tcp::State::Listen | tcp::State::SynSent => {
                            // No state changes.
                        }
                        tcp::State::SynReceived => {
                            let State::TcpListener {
                                pending_accepts, ..
                            } = &mut *state_borrow
                            else {
                                unreachable!();
                            };

                            // Check if we can accept a new connection.
                            if let Some(completer) = pending_accepts.pop_front() {
                                accepted_sockets.push((
                                    handle,
                                    socket.listen_endpoint(),
                                    completer,
                                ));
                            }
                        }
                        tcp::State::Established | tcp::State::FinWait1 | tcp::State::FinWait2 => {
                            tcp_read_write(socket, &mut state_borrow);
                        }
                        tcp::State::CloseWait => {
                            tcp_read_write(socket, &mut state_borrow);
                            tcp_peer_closed(socket, &mut state_borrow);
                        }
                        tcp::State::Closing | tcp::State::LastAck => {
                            // Waiting for the peer to acknowledge the close.
                        }
                        tcp::State::TimeWait | tcp::State::Closed => {
                            // The socket has been closed by both sides.
                            let State::TcpConn { channel_id, .. } = &mut *state_borrow else {
                                unreachable!();
                            };
                            destroyed_sockets.push((handle, *channel_id));
                        }
                    }
                }
            }
        }

        for (handle, endpoint, completer) in accepted_sockets {
            match self.tcp_accept(ctx, handle, endpoint) {
                Ok(new_ch) => completer.complete(new_ch),
                Err(error) => completer.error(error),
            }
        }

        for (handle, channel_id) in destroyed_sockets {
            self.sockets.remove(handle);
            self.states_by_handle.remove(&handle);
            if let Err(error) = ctx.remove(channel_id) {
                println!("failed to remove channel: {:?}", error);
            }
        }
    }
}

fn tcp_read_write(socket: &mut tcp::Socket, state: &mut State) {
    let State::TcpConn {
        pending_reads,
        pending_writes,
        ..
    } = state
    else {
        unreachable!();
    };

    while socket.can_recv() {
        let Some(completer) = pending_reads.pop_front() else {
            break;
        };
        socket.recv(|buf| {
            // Documentation:
            //
            // > Call f with the largest contiguous slice of octets in the receive
            // > buffer, and dequeue the amount of elements returned by f.
            let read_len = match completer.write_data(0, buf) {
                Ok(len) => {
                    // TODO: Reuse the completer if the entire buffer was written.
                    completer.complete(len);
                    len
                }
                Err(error) => {
                    println!("failed to write data to read completer: {:?}", error);
                    completer.error(error);
                    0
                }
            };

            (read_len, () /* retrun value of recv */)
        });
    }

    while socket.can_send() {
        let Some(completer) = pending_writes.pop_front() else {
            break;
        };
        socket.send(|buf| {
            // Documentation:
            //
            // > Call f with the largest contiguous slice of octets in the
            // > transmit buffer, and enqueue the amount of elements returned
            // > by f.
            let write_len = match completer.read_data(0, buf) {
                Ok(len) => {
                    // TODO: Reuse the completer if the entire buffer was written.
                    completer.complete(len);
                    len
                }
                Err(error) => {
                    println!("failed to read data from write completer: {:?}", error);
                    completer.error(error);
                    0
                }
            };

            (write_len, () /* return value of send */)
        });
    }
}

fn tcp_peer_closed(socket: &mut tcp::Socket, state: &mut State) {
    let State::TcpConn {
        pending_reads,
        pending_writes,
        channel_closed,
        ..
    } = state
    else {
        unreachable!();
    };

    if !*channel_closed {
        // Keep the socket open until the channel is closed so we can continue
        // sending data after the peer's FIN (half-close).
        return;
    }

    // No more data to read since the peer closed the connection. Complete
    // all pending reads.
    debug_assert!(!socket.can_recv());
    for completer in pending_reads.drain(..) {
        completer.complete(0);
    }

    if !pending_writes.is_empty() {
        debug_assert!(!socket.can_send());
        // We still have pending writes to send. Do not close the socket yet.
        return;
    }

    debug_assert!(pending_reads.is_empty());
    debug_assert!(pending_writes.is_empty());

    // It's safe to close the socket now. Send a FIN packet to the peer.
    socket.close();
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
        let hwaddr = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let gw_ip = Ipv4Address::new(10, 0, 2, 2);
        let our_ip = IpCidr::Ipv4(Ipv4Cidr::new(Ipv4Address::new(10, 0, 2, 15), 24));

        let smol_clock = SmolClock::new();
        let hwaddr = HardwareAddress::Ethernet(EthernetAddress::from_bytes(&hwaddr));
        let config = smoltcp::iface::Config::new(hwaddr);

        let mut device = Device::new();
        ctx.add_channel(device.channel()).unwrap();

        let mut iface = Interface::new(config, &mut device, smol_clock.now());

        iface.routes_mut().add_default_ipv4_route(gw_ip).unwrap();
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs.push(our_ip).unwrap();
        });

        Self {
            smol_clock,
            sockets: SocketSet::new(Vec::new()),
            states_by_ch: HashMap::new(),
            states_by_handle: HashMap::new(),
            device,
            iface: iface,
        }
    }

    fn open(&mut self, ctx: &mut Context, completer: OpenCompleter) {
        let Some(state) = self.states_by_ch.get(&ctx.handle_id()) else {
            completer.error(ErrorCode::InvalidArgument);
            return;
        };

        let mut state_borrow = state.borrow_mut();
        match &mut *state_borrow {
            State::TcpListener {
                pending_accepts, ..
            } => {
                pending_accepts.push_back(completer);
            }
            State::Control => {
                drop(state_borrow);
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
            State::TcpConn { .. } | State::Driver => {
                completer.error(ErrorCode::Unsupported);
            }
        }
    }

    fn read(&mut self, ctx: &mut Context, completer: ReadCompleter, offset: usize, len: usize) {
        let Some(state) = self.states_by_ch.get(&ctx.handle_id()) else {
            println!("state not found for {:?}", ctx.handle_id());
            completer.error(ErrorCode::InvalidArgument);
            return;
        };

        let mut state_borrow = state.borrow_mut();
        match &mut *state_borrow {
            State::TcpConn { pending_reads, .. } => {
                pending_reads.push_back(completer);
                drop(state_borrow);
                self.poll(ctx);
            }
            State::TcpListener { .. } => {
                completer.error(ErrorCode::Unsupported);
            }
            State::Driver | State::Control => {
                completer.error(ErrorCode::Unsupported);
            }
        }
    }

    fn write(&mut self, ctx: &mut Context, completer: WriteCompleter, offset: usize, len: usize) {
        let Some(state) = self.states_by_ch.get(&ctx.handle_id()) else {
            println!("state not found for {:?}", ctx.handle_id());
            completer.error(ErrorCode::InvalidArgument);
            return;
        };

        let mut state_borrow = state.borrow_mut();
        match &mut *state_borrow {
            State::TcpConn { pending_writes, .. } => {
                pending_writes.push_back(completer);
                drop(state_borrow);
                self.poll(ctx);
            }
            State::TcpListener { .. } => {
                completer.error(ErrorCode::Unsupported);
            }
            State::Driver | State::Control => {
                completer.error(ErrorCode::Unsupported);
            }
        }
    }

    fn read_reply(&mut self, ctx: &mut Context, _ch: &Rc<Channel>, buf: BufferMut, len: usize) {
        let mut state = self
            .states_by_ch
            .get(&ctx.handle_id())
            .unwrap()
            .borrow_mut();

        match &mut *state {
            State::Driver => {
                drop(state);
                self.device.on_read_reply(buf, len);
                self.poll(ctx);
            }
            _ => {
                println!("unexpected read reply");
            }
        }
    }

    fn write_reply(&mut self, ctx: &mut Context, _ch: &Rc<Channel>, _buf: Buffer, _len: usize) {
        if ctx.handle_id() == self.device.handle_id() {
            return;
        }

        println!("unexpected write reply on {:?}", ctx.handle_id());
    }

    fn peer_closed(&mut self, ctx: &mut Context, _ch: &Rc<Channel>) {
        let Some(state) = self.states_by_ch.get(&ctx.handle_id()) else {
            println!("state not found for {:?}", ctx.handle_id());
            return;
        };

        let mut state_borrow = state.borrow_mut();
        match &mut *state_borrow {
            State::TcpConn {
                pending_reads,
                pending_writes,
                channel_closed,
                ..
            } => {
                *channel_closed = true;
            }
            State::TcpListener {
                pending_accepts, ..
            } => {
                // Nothing t odo.
            }
            State::Driver => {
                todo!("handle driver peer closed");
            }
            State::Control => {
                // Nothing to do.
            }
        }

        drop(state_borrow);
        self.poll(ctx);
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
