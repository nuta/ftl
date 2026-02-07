#![no_std]
#![no_main]

use core::cell::RefCell;
use core::fmt;
use core::net::Ipv4Addr;

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
use ftl::log::*;
use ftl::prelude::*;
use ftl::rc::Rc;
use ftl::time::Timer;
use smoltcp::iface::Interface;
use smoltcp::iface::PollResult;
use smoltcp::iface::SocketHandle;
use smoltcp::iface::SocketSet;
use smoltcp::phy::DeviceCapabilities;
use smoltcp::socket::dhcpv4;
use smoltcp::socket::tcp;
use smoltcp::socket::tcp::ListenError;
use smoltcp::wire::EthernetAddress;
use smoltcp::wire::HardwareAddress;
use smoltcp::wire::IpCidr;
use smoltcp::wire::IpListenEndpoint;
use smoltcp::wire::Ipv4Cidr;

enum Uri {
    TcpListen(IpListenEndpoint),
}

const TCP_BUFFER_SIZE: usize = 4096;
const NET_RX_BUFFER_SIZE: usize = 1514;
const RX_QUEUE_SIZE: usize = 1;
const VIRTIO_NET_MAC_URI: &[u8] = b"ethernet:mac";

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
            trace!("failed to send packet: {:?}", error);
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
    fn new(ch: Rc<Channel>) -> Self {
        Self {
            ch,
            rx_queue: VecDeque::new(),
            inflight_reads: 0,
        }
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
                    trace!("failed to send a packet to drivers: {:?}", error);
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
        _timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(packet) = self.rx_queue.pop_front() {
            // Keep one RX read in flight after consuming a packet so the next
            // incoming frame can trigger a read reply immediately.
            self.fill_rx();
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

    fn transmit(&mut self, _timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
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
    DriverMac,
    Control,
    TcpConn {
        pending_reads: VecDeque<ReadCompleter>,
        pending_writes: VecDeque<WriteCompleter>,
        channel_id: HandleId,
        channel_closed: bool,
    },
    TcpListener {
        pending_accepts: VecDeque<OpenCompleter>,
    },
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Driver => write!(f, "Driver"),
            State::DriverMac => write!(f, "DriverMac"),
            State::Control => write!(f, "Control"),
            State::TcpConn { .. } => write!(f, "TcpConn"),
            State::TcpListener { .. } => write!(f, "TcpListener"),
        }
    }
}

struct SmolClock {
    started_at: ftl::time::Instant,
}

impl SmolClock {
    fn new() -> Self {
        Self {
            started_at: ftl::time::Instant::now(),
        }
    }

    fn now(&self) -> smoltcp::time::Instant {
        let elapsed = ftl::time::Instant::now().elapsed_since(&self.started_at);
        let elapsed_micros = elapsed.as_micros().min(i64::MAX as u128) as i64;
        smoltcp::time::Instant::from_micros(elapsed_micros)
    }
}

struct Main {
    smol_clock: SmolClock,
    timer: Rc<Timer>,
    sockets: SocketSet<'static>,
    states_by_ch: HashMap<HandleId, Rc<RefCell<State>>>,
    states_by_handle: HashMap<SocketHandle, Rc<RefCell<State>>>,
    dhcp_handle: SocketHandle,
    device: Device,
    iface: Interface,
    ready_to_serve: bool,
}

impl Main {
    fn update_timer(&mut self) {
        let now = self.smol_clock.now();
        let Some(delay) = self.iface.poll_delay(now, &self.sockets) else {
            return;
        };

        if let Err(error) = self.timer.set_timeout(delay.into()) {
            trace!("failed to set poll timer: {:?}", error);
        }
    }

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
                trace!("unexpected listen error: {:?}", e);
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
        ctx.add_channel(our_ch).unwrap();

        // Create a new listen socket.
        let new_listen_handle = self.do_tcp_listen(endpoint)?;

        let conn_state = Rc::new(RefCell::new(State::TcpConn {
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
        if !self.ready_to_serve {
            return;
        }

        loop {
            let now = self.smol_clock.now();
            let result = self.iface.poll(now, &mut self.device, &mut self.sockets);
            self.poll_dhcp();
            self.do_poll(ctx);
            if matches!(result, PollResult::SocketStateChanged) {
                continue;
            }

            // We may have queued data while handling sockets. Poll once more to flush.
            let now = self.smol_clock.now();
            let result = self.iface.poll(now, &mut self.device, &mut self.sockets);
            if matches!(result, PollResult::SocketStateChanged) {
                continue;
            }
            break;
        }

        self.update_timer();
    }

    fn poll_dhcp(&mut self) {
        let event = self
            .sockets
            .get_mut::<dhcpv4::Socket>(self.dhcp_handle)
            .poll();

        match event {
            None => {}
            Some(dhcpv4::Event::Configured(config)) => {
                let our_ip = config.address;
                let gw_ip = config.router;
                self.apply_dhcp_config(our_ip, gw_ip);
            }
            Some(dhcpv4::Event::Deconfigured) => {
                trace!("DHCP deconfigured");
            }
        }
    }

    fn apply_dhcp_config(&mut self, mut our_ip: Ipv4Cidr, gw_ip: Option<Ipv4Addr>) {
        trace!("DHCP configured: address={}, router={:?}", our_ip, gw_ip);

        // Google Compute Engine assigns a /32 address, which confuses
        // smoltcp since it's not in the same subnet as the router.
        //
        // Adjust the address to the common prefix with the router.
        if let Some(gw_ip) = gw_ip {
            if !our_ip.contains_addr(&gw_ip) {
                // Compute the common prefix between the address and the router.
                let a = u32::from_be_bytes(our_ip.address().octets());
                let b = u32::from_be_bytes(gw_ip.octets());
                let prefix = (a ^ b).leading_zeros() as u8;

                let adjusted = Ipv4Cidr::new(our_ip.address(), prefix);
                trace!(
                    "adjusting IPv4 prefix: {} -> {} (router {})",
                    our_ip, adjusted, gw_ip
                );
                our_ip = adjusted;
            }
        }

        // Set our IP address.
        self.iface.update_ip_addrs(|addrs| {
            addrs.clear();
            addrs.push(IpCidr::Ipv4(our_ip)).unwrap();
        });

        // Set the default route.
        if let Some(gw_ip) = gw_ip {
            if let Err(error) = self.iface.routes_mut().add_default_ipv4_route(gw_ip) {
                trace!("failed to add default IPv4 route: {:?}", error);
            }
        } else {
            trace!("missing default IPv4 route");
            self.iface.routes_mut().remove_default_ipv4_route();
        }
    }

    fn do_poll(&mut self, ctx: &mut Context) {
        use smoltcp::socket::Socket;

        let mut accepted_sockets = Vec::new();
        let mut destroyed_sockets = Vec::new();
        for (handle, socket) in self.sockets.iter_mut() {
            match socket {
                Socket::Dhcpv4(_socket) => {
                    // DHCP socket is handled in poll_dhcp.
                }
                Socket::Tcp(socket) => {
                    let state = self.states_by_handle.get(&handle).unwrap();
                    let mut state_borrow = state.borrow_mut();
                    match socket.state() {
                        tcp::State::Listen | tcp::State::SynSent => {
                            // No state changes.
                        }
                        tcp::State::SynReceived => {
                            match &mut *state_borrow {
                                State::TcpListener {
                                    pending_accepts, ..
                                } => {
                                    // Check if we can accept a new connection.
                                    if let Some(completer) = pending_accepts.pop_front() {
                                        accepted_sockets.push((
                                            handle,
                                            socket.listen_endpoint(),
                                            completer,
                                        ));
                                    }
                                }
                                State::TcpConn { .. } => {
                                    // Handshake in progress for an accepted socket.
                                }
                                _ => {
                                    trace!("unexpected state: {:?}", *state_borrow);
                                    unreachable!();
                                }
                            }
                        }
                        tcp::State::Established => {
                            tcp_read_write(socket, &mut state_borrow);
                            tcp_channel_closed(socket, &mut state_borrow);
                        }
                        tcp::State::FinWait1 | tcp::State::FinWait2 => {
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
            self.states_by_ch.remove(&channel_id);
            if let Err(error) = ctx.remove(channel_id) {
                trace!("failed to remove channel: {:?}", error);
            }
        }
    }

    fn on_mac_read_reply(&mut self, buf: BufferMut, len: usize) {
        let data = match buf {
            BufferMut::Vec(mut data) => {
                data.truncate(len.min(data.len()));
                data
            }
            _ => unreachable!(),
        };

        if data.len() < 6 {
            trace!("MAC reply too short: {} bytes", data.len());
            return;
        }

        let mac = [data[0], data[1], data[2], data[3], data[4], data[5]];
        let hwaddr = HardwareAddress::Ethernet(EthernetAddress::from_bytes(&mac));
        self.iface.set_hardware_addr(hwaddr);
        self.ready_to_serve = true;
        trace!(
            "MAC configured: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );
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
        let result = socket.recv(|buf| {
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
                    trace!("failed to write data to read completer: {:?}", error);
                    completer.error(error);
                    0
                }
            };

            (read_len, () /* retrun value of recv */)
        });

        if let Err(error) = result {
            warn!("failed to recv from socket: {:?}", error);
            break;
        }
    }

    while socket.can_send() {
        let Some(completer) = pending_writes.pop_front() else {
            break;
        };

        let result = socket.send(|buf| {
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
                    trace!("failed to read data from write completer: {:?}", error);
                    completer.error(error);
                    0
                }
            };

            (write_len, () /* return value of send */)
        });

        if let Err(error) = result {
            warn!("failed to write to socket: {:?}", error);
            break;
        }
    }
}

// TODO: Merge this into tcp_read_write?
fn tcp_channel_closed(socket: &mut tcp::Socket, state: &mut State) {
    let State::TcpConn {
        pending_writes,
        channel_closed,
        ..
    } = state
    else {
        unreachable!();
    };

    if !*channel_closed {
        return;
    }

    if !pending_writes.is_empty() {
        // Still have application data to enqueue.
        return;
    }

    // Initiate a local close (FIN) when the channel is closed.
    trace!("initiating FIN (channel closed)");
    socket.close();
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
    trace!("closing socket (peer closed)");
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
        trace!("starting...");
        let smol_clock = SmolClock::new();
        let hwaddr = HardwareAddress::Ethernet(EthernetAddress::from_bytes(&[0; 6]));
        let config = smoltcp::iface::Config::new(hwaddr);

        let driver_ch = Rc::new(Channel::from_handle(OwnedHandle::from_raw(
            HandleId::from_raw(1),
        )));
        let http_ch = Rc::new(Channel::from_handle(OwnedHandle::from_raw(
            HandleId::from_raw(2),
        )));

        let mut states_by_ch = HashMap::new();

        let driver_id = driver_ch.handle().id();
        let http_id = http_ch.handle().id();
        let driver_state = Rc::new(RefCell::new(State::Driver));
        let http_state = Rc::new(RefCell::new(State::Control));
        states_by_ch.insert(driver_id, driver_state.clone());
        states_by_ch.insert(http_id, http_state.clone());

        ctx.add_channel(driver_ch.clone()).unwrap();
        ctx.add_channel(http_ch.clone()).unwrap();

        if let Err(error) = driver_ch.send(Message::Open {
            uri: Buffer::Static(VIRTIO_NET_MAC_URI),
        }) {
            trace!("failed to request MAC: {:?}", error);
        }
        let mut device = Device::new(driver_ch);
        let timer = Rc::new(Timer::new().expect("failed to create poll timer"));
        ctx.add_timer(timer.clone()).unwrap();

        let iface = Interface::new(config, &mut device, smol_clock.now());

        let mut sockets = SocketSet::new(Vec::new());
        let dhcp_handle = sockets.add(dhcpv4::Socket::new());

        let mut this = Self {
            smol_clock,
            timer,
            sockets,
            states_by_ch,
            states_by_handle: HashMap::new(),
            dhcp_handle,
            device,
            iface: iface,
            ready_to_serve: false,
        };

        this.poll(ctx);
        this
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
                        trace!("invalid URI: {:?}", error);
                        completer.error(ErrorCode::InvalidArgument)
                    }
                }
            }
            State::TcpConn { .. } | State::Driver | State::DriverMac => {
                completer.error(ErrorCode::Unsupported);
            }
        }
    }

    fn read(&mut self, ctx: &mut Context, completer: ReadCompleter, _offset: usize, _len: usize) {
        let Some(state) = self.states_by_ch.get(&ctx.handle_id()) else {
            trace!("state not found for read on {:?}", ctx.handle_id());
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
            State::Driver | State::DriverMac | State::Control => {
                completer.error(ErrorCode::Unsupported);
            }
        }
    }

    fn write(&mut self, ctx: &mut Context, completer: WriteCompleter, _offset: usize, _len: usize) {
        let Some(state) = self.states_by_ch.get(&ctx.handle_id()) else {
            trace!("state not found for write on {:?}", ctx.handle_id());
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
            State::Driver | State::DriverMac | State::Control => {
                completer.error(ErrorCode::Unsupported);
            }
        }
    }

    fn open_reply(&mut self, ctx: &mut Context, _ch: &Rc<Channel>, _uri: Buffer, new_ch: Channel) {
        let ch_id = ctx.handle_id();
        let state = self.states_by_ch.get(&ch_id).unwrap().borrow_mut();
        match &*state {
            State::Driver => {
                let new_ch = Rc::new(new_ch);
                if let Err(error) = ctx.add_channel(new_ch.clone()) {
                    trace!("failed to add MAC control channel: {:?}", error);
                    return;
                }

                drop(state);
                let new_id = new_ch.handle().id();
                self.states_by_ch
                    .insert(new_id, Rc::new(RefCell::new(State::DriverMac)));

                if let Err(error) = new_ch.send(Message::Read {
                    offset: 0,
                    data: BufferMut::Vec(vec![0u8; 6]),
                }) {
                    trace!("failed to read MAC: {:?}", error);
                }
            }
            _ => {
                trace!("unexpected state for open reply: {state:?}");
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
            State::DriverMac => {
                drop(state);
                self.on_mac_read_reply(buf, len);
                self.poll(ctx);
            }
            _ => {
                trace!("unexpected read reply");
            }
        }
    }

    fn write_reply(&mut self, ctx: &mut Context, _ch: &Rc<Channel>, _buf: Buffer, _len: usize) {
        let Some(state) = self.states_by_ch.get(&ctx.handle_id()) else {
            trace!("state not found for write reply on {:?}", ctx.handle_id());
            return;
        };

        let mut state_borrow = state.borrow_mut();
        match &mut *state_borrow {
            State::Driver => {
                // Sent a packet.
            }
            _ => {
                trace!("unexpected write reply on {:?}", ctx.handle_id());
            }
        }
    }

    fn peer_closed(&mut self, ctx: &mut Context, _ch: &Rc<Channel>) {
        let Some(state) = self.states_by_ch.get(&ctx.handle_id()) else {
            trace!("state not found for peer closed on {:?}", ctx.handle_id());
            return;
        };

        let mut state_borrow = state.borrow_mut();
        match &mut *state_borrow {
            State::TcpConn { channel_closed, .. } => {
                *channel_closed = true;
            }
            State::TcpListener { .. } => {
                // Nothing t odo.
                ctx.remove(ctx.handle_id()).unwrap();
            }
            State::Driver => {
                todo!("handle driver peer closed");
            }
            State::DriverMac => {
                trace!("MAC control channel closed");
                ctx.remove(ctx.handle_id()).unwrap();
            }
            State::Control => {
                // Nothing to do.
                ctx.remove(ctx.handle_id()).unwrap();
            }
        }

        drop(state_borrow);
        self.poll(ctx);
    }

    fn timer_expired(&mut self, ctx: &mut Context, _timer: &Rc<Timer>) {
        trace!("timer expired");
        self.poll(ctx);
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
