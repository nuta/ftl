#![no_std]
#![no_main]

use core::fmt;
use core::net::Ipv4Addr;

use ftl::buffer::Buffer;
use ftl::buffer::BufferMut;
use ftl::buffer::BufferUninit;
use ftl::channel::Attr;
use ftl::channel::Channel;
use ftl::collections::HashMap;
use ftl::collections::VecDeque;
use ftl::error::ErrorCode;
use ftl::eventloop::Client;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::eventloop::OpenRequest;
use ftl::eventloop::ReadRequest;
use ftl::eventloop::Reply;
use ftl::eventloop::Request;
use ftl::eventloop::WriteRequest;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
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
type AppEventLoop = EventLoop<(), ()>;

// TODO: Remove this default timeout once we solve leaks.
const TCP_SOCKET_TIMEOUT: smoltcp::time::Duration = smoltcp::time::Duration::from_secs(30);

struct RxToken {
    buffer: Buffer,
}

impl smoltcp::phy::RxToken for RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.buffer)
    }
}

struct TxToken {
    ch: Rc<Channel>,
}

impl smoltcp::phy::TxToken for TxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buf = vec![0u8; len];
        let result = f(&mut buf);

        if let Err(error) = Client::new(self.ch).write(0, buf, ()) {
            // TODO: Add a semaphore to limit the number of inflight writes.
            trace!("failed to send packet: {:?}", error);
        }

        result
    }
}

struct Device {
    ch: Rc<Channel>,
    rx_queue: VecDeque<Buffer>,
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

    fn on_read_reply(&mut self, buf: Buffer, len: usize) {
        debug_assert!(self.inflight_reads > 0);
        self.inflight_reads -= 1;

        if len == 0 {
            self.fill_rx();
            return;
        }

        self.rx_queue.push_back(buf);
        self.fill_rx();
    }

    fn fill_rx(&mut self) {
        while self.rx_queue.len() + self.inflight_reads < RX_QUEUE_SIZE {
            let uninit = BufferUninit::with_capacity(NET_RX_BUFFER_SIZE);
            match Client::new(self.ch.clone()).read(0, uninit, ()) {
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
    type TxToken<'a> = TxToken;

    fn receive(
        &mut self,
        _timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(buffer) = self.rx_queue.pop_front() {
            // Keep one RX read in flight after consuming a packet so the next
            // incoming frame can trigger a read reply immediately.
            self.fill_rx();
            let rx = RxToken { buffer };
            let tx = TxToken {
                ch: self.ch.clone(),
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
            ch: self.ch.clone(),
        })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = smoltcp::phy::Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps
    }
}

enum Context {
    Driver,
    ServiceListener,
    ControlClient,
    TcpConn {
        pending_reads: VecDeque<ReadRequest>,
        pending_writes: VecDeque<WriteRequest>,
        channel_closed: bool,
    },
    TcpListener {
        pending_accepts: VecDeque<OpenRequest>,
    },
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Context::Driver => write!(f, "Driver"),
            Context::ServiceListener => write!(f, "ServiceListener"),
            Context::ControlClient => write!(f, "ControlClient"),
            Context::TcpConn { .. } => write!(f, "TcpConn"),
            Context::TcpListener { .. } => write!(f, "TcpListener"),
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
    contexts: HashMap<HandleId, Context>,
    socket2id: HashMap<SocketHandle, HandleId>,
    dhcp_handle: SocketHandle,
    device: Device,
    iface: Interface,
    ready_to_serve: bool,
}

impl Main {
    fn connect_driver(&mut self, eventloop: &mut AppEventLoop) -> Result<(), ErrorCode> {
        let driver_ch = Rc::new(Channel::connect("ethernet")?);
        eventloop.add_channel(driver_ch.clone(), ())?;

        let driver_id = driver_ch.handle().id();
        self.contexts.insert(driver_id, Context::Driver);

        self.device.ch = driver_ch.clone();
        self.device.rx_queue.clear();
        self.device.inflight_reads = 0;
        self.ready_to_serve = false;

        let uninit = BufferUninit::with_capacity(6);
        if let Err(error) = Client::new(driver_ch.clone()).getattr(Attr::MAC, uninit, ()) {
            trace!("failed to request MAC: {:?}", error);
        }

        Ok(())
    }

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
        socket.set_timeout(Some(TCP_SOCKET_TIMEOUT));

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
        eventloop: &mut AppEventLoop,
        endpoint: IpListenEndpoint,
    ) -> Result<Channel, ErrorCode> {
        let (our_ch, their_ch) = Channel::new()?;
        let handle = self.do_tcp_listen(endpoint)?;
        let ch_id = our_ch.handle().id();
        eventloop.add_channel(our_ch, ())?;

        self.contexts.insert(
            ch_id,
            Context::TcpListener {
                pending_accepts: VecDeque::new(),
            },
        );
        self.socket2id.insert(handle, ch_id);
        Ok(their_ch)
    }

    pub fn tcp_accept(
        &mut self,
        eventloop: &mut AppEventLoop,
        accepted_handle: SocketHandle,
        endpoint: IpListenEndpoint,
    ) -> Result<Channel, ErrorCode> {
        let (our_ch, their_ch) = Channel::new()?;
        let new_ch_id = our_ch.handle().id();
        eventloop.add_channel(our_ch, ())?;

        // Create a new listen socket.
        let new_listen_handle = self.do_tcp_listen(endpoint)?;

        self.contexts.insert(
            new_ch_id,
            Context::TcpConn {
                pending_reads: VecDeque::new(),
                pending_writes: VecDeque::new(),
                channel_closed: false,
            },
        );

        // Replace the accepted socket's state mapping.
        let listen_ch_id = self.socket2id.insert(accepted_handle, new_ch_id).unwrap();
        self.socket2id.insert(new_listen_handle, listen_ch_id);

        Ok(their_ch)
    }

    pub fn poll(&mut self, eventloop: &mut AppEventLoop) {
        if !self.ready_to_serve {
            return;
        }

        loop {
            let now = self.smol_clock.now();
            let result = self.iface.poll(now, &mut self.device, &mut self.sockets);
            self.poll_dhcp();
            self.do_poll(eventloop);
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
        trace!(
            "poll completed: sockets={}, states={}",
            self.sockets.iter().count(),
            self.contexts.len()
        );
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

    fn do_poll(&mut self, eventloop: &mut AppEventLoop) {
        use smoltcp::socket::Socket;

        let mut accepted_sockets = Vec::new();
        let mut destroyed_sockets = Vec::new();
        for (handle, socket) in self.sockets.iter_mut() {
            match socket {
                Socket::Dhcpv4(_socket) => {
                    // DHCP socket is handled in poll_dhcp.
                }
                Socket::Tcp(socket) => {
                    let Some(id) = self.socket2id.get(&handle).copied() else {
                        trace!("state mapping not found for socket {:?}", handle);
                        continue;
                    };
                    let Some(state) = self.contexts.get_mut(&id) else {
                        trace!(
                            "context not found for socket {:?} (channel {:?})",
                            handle, id
                        );
                        destroyed_sockets.push((handle, id));
                        continue;
                    };
                    match socket.state() {
                        tcp::State::Listen | tcp::State::SynSent => {
                            // No state changes.
                        }
                        tcp::State::SynReceived => {
                            match state {
                                Context::TcpListener {
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
                                Context::TcpConn { .. } => {
                                    // Handshake in progress for an accepted socket.
                                }
                                _ => {
                                    trace!("unexpected state: {:?}", state);
                                    unreachable!();
                                }
                            }
                        }
                        tcp::State::Established => {
                            tcp_read_write(socket, state);
                            tcp_channel_closed(socket, state);
                        }
                        tcp::State::FinWait1 | tcp::State::FinWait2 => {
                            tcp_read_write(socket, state);
                        }
                        tcp::State::CloseWait => {
                            tcp_read_write(socket, state);
                            tcp_peer_closed(socket, state);
                        }
                        tcp::State::Closing | tcp::State::LastAck => {
                            // Waiting for the peer to acknowledge the close.
                        }
                        tcp::State::TimeWait | tcp::State::Closed => {
                            // The socket has been closed by both sides.
                            let Context::TcpConn { .. } = state else {
                                unreachable!();
                            };
                            destroyed_sockets.push((handle, id));
                        }
                    }
                }
            }
        }

        for (handle, endpoint, request) in accepted_sockets {
            match self.tcp_accept(eventloop, handle, endpoint) {
                Ok(new_ch) => request.reply(new_ch),
                Err(error) => request.reply_error(error),
            }
        }

        for (handle, channel_id) in destroyed_sockets {
            self.sockets.remove(handle);
            self.socket2id.remove(&handle);
            self.contexts.remove(&channel_id);
            eventloop.remove(channel_id);
        }
    }

    fn on_mac_read_reply(&mut self, data: BufferMut, len: usize) {
        if len != 6 {
            warn!("unexpected MAC length: {}", len);
            return;
        }

        let data = &data[..len];
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

fn tcp_read_write(socket: &mut tcp::Socket, state: &mut Context) {
    let Context::TcpConn {
        pending_reads,
        pending_writes,
        ..
    } = state
    else {
        unreachable!();
    };

    while socket.can_recv() {
        let Some(request) = pending_reads.pop_front() else {
            break;
        };
        let result = socket.recv(|buf| {
            // Documentation:
            //
            // > Call f with the largest contiguous slice of octets in the receive
            // > buffer, and dequeue the amount of elements returned by f.
            let read_len = match request.write(buf) {
                Ok(len) => {
                    // TODO: Reuse the request if the entire buffer was written.
                    request.reply(len);
                    len
                }
                Err(error) => {
                    trace!("failed to write data to read request: {:?}", error);
                    request.reply_error(error);
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
        let Some(request) = pending_writes.pop_front() else {
            break;
        };

        let result = socket.send(|buf| {
            // Documentation:
            //
            // > Call f with the largest contiguous slice of octets in the
            // > transmit buffer, and enqueue the amount of elements returned
            // > by f.
            let write_len = match request.read(buf) {
                Ok(len) => {
                    // TODO: Reuse the request if the entire buffer was written.
                    request.reply(len);
                    len
                }
                Err(error) => {
                    trace!("failed to read data from write request: {:?}", error);
                    request.reply_error(error);
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
fn tcp_channel_closed(socket: &mut tcp::Socket, state: &mut Context) {
    let Context::TcpConn {
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

fn tcp_peer_closed(socket: &mut tcp::Socket, state: &mut Context) {
    let Context::TcpConn {
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
    for request in pending_reads.drain(..) {
        request.reply(0);
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

fn parse_uri(request: &OpenRequest) -> Result<Uri, ErrorCode> {
    let mut buf = [0; 256];
    let len = request.path(&mut buf)?;

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

impl Main {
    fn new(eventloop: &mut AppEventLoop) -> Self {
        trace!("starting...");
        let smol_clock = SmolClock::new();
        let hwaddr = HardwareAddress::Ethernet(EthernetAddress::from_bytes(&[0; 6]));
        let config = smoltcp::iface::Config::new(hwaddr);

        let driver_ch = Rc::new(Channel::connect("ethernet").unwrap());
        let mut states = HashMap::new();

        let driver_id = driver_ch.handle().id();
        states.insert(driver_id, Context::Driver);

        eventloop.add_channel(driver_ch.clone(), ()).unwrap();

        if let Err(error) =
            Client::new(driver_ch.clone()).getattr(Attr::MAC, BufferUninit::with_capacity(6), ())
        {
            trace!("failed to request MAC: {:?}", error);
        }

        let mut device = Device::new(driver_ch);
        let timer = Rc::new(Timer::new().expect("failed to create poll timer"));
        eventloop.add_timer(timer.clone(), ()).unwrap();

        let iface = Interface::new(config, &mut device, smol_clock.now());

        let mut sockets = SocketSet::new(Vec::new());
        let dhcp_handle = sockets.add(dhcpv4::Socket::new());

        let service_ch = Rc::new(Channel::register("tcpip").unwrap());
        let service_id = service_ch.handle().id();
        eventloop.add_channel(service_ch, ()).unwrap();
        states.insert(service_id, Context::ServiceListener);

        trace!("ready");
        Self {
            smol_clock,
            timer,
            sockets,
            contexts: states,
            socket2id: HashMap::new(),
            dhcp_handle,
            device,
            iface: iface,
            ready_to_serve: false,
        }
    }

    fn on_open_request(&mut self, eventloop: &mut AppEventLoop, request: OpenRequest) {
        let handle_id = request.handle_id();
        match self.contexts.get_mut(&handle_id) {
            Some(Context::TcpListener {
                pending_accepts, ..
            }) => {
                pending_accepts.push_back(request);
                return;
            }
            Some(Context::ServiceListener) => {
                let (our_ch, their_ch) = match Channel::new() {
                    Ok(ch) => ch,
                    Err(error) => {
                        request.reply_error(error);
                        return;
                    }
                };

                let our_id = our_ch.handle().id();
                if let Err(error) = eventloop.add_channel(our_ch, ()) {
                    request.reply_error(error.into());
                    return;
                }

                self.contexts.insert(our_id, Context::ControlClient);
                request.reply(their_ch);
                return;
            }
            Some(Context::ControlClient) => {}
            Some(Context::TcpConn { .. }) | Some(Context::Driver) => {
                request.reply_error(ErrorCode::Unsupported);
                return;
            }
            None => {
                request.reply_error(ErrorCode::InvalidArgument);
                return;
            }
        }

        match parse_uri(&request) {
            Ok(Uri::TcpListen(endpoint)) => {
                match self.tcp_listen(eventloop, endpoint) {
                    Ok(new_ch) => request.reply(new_ch),
                    Err(error) => request.reply_error(error),
                }
            }
            Err(error) => {
                trace!("invalid URI: {:?}", error);
                request.reply_error(ErrorCode::InvalidArgument)
            }
        }
    }

    fn on_read_request(&mut self, eventloop: &mut AppEventLoop, request: ReadRequest) {
        let handle_id = request.handle_id();
        let mut should_poll = false;
        match self.contexts.get_mut(&handle_id) {
            Some(Context::TcpConn { pending_reads, .. }) => {
                pending_reads.push_back(request);
                should_poll = true;
            }
            Some(Context::TcpListener { .. }) => {
                request.reply_error(ErrorCode::Unsupported);
            }
            Some(Context::Driver)
            | Some(Context::ServiceListener)
            | Some(Context::ControlClient) => {
                request.reply_error(ErrorCode::Unsupported);
            }
            None => {
                trace!("state not found for read on {:?}", handle_id);
                request.reply_error(ErrorCode::InvalidArgument);
            }
        }

        if should_poll {
            self.poll(eventloop);
        }
    }

    fn on_write_request(&mut self, eventloop: &mut AppEventLoop, request: WriteRequest) {
        let handle_id = request.handle_id();
        let mut should_poll = false;
        match self.contexts.get_mut(&handle_id) {
            Some(Context::TcpConn { pending_writes, .. }) => {
                pending_writes.push_back(request);
                should_poll = true;
            }
            Some(Context::TcpListener { .. }) => {
                request.reply_error(ErrorCode::Unsupported);
            }
            Some(Context::Driver)
            | Some(Context::ServiceListener)
            | Some(Context::ControlClient) => {
                request.reply_error(ErrorCode::Unsupported);
            }
            None => {
                trace!("state not found for write on {:?}", handle_id);
                request.reply_error(ErrorCode::InvalidArgument);
            }
        }

        if should_poll {
            self.poll(eventloop);
        }
    }

    fn on_read_reply(
        &mut self,
        eventloop: &mut AppEventLoop,
        ch: &Rc<Channel>,
        buf: BufferMut,
        len: usize,
    ) {
        let handle_id = ch.handle().id();
        match self.contexts.get(&handle_id) {
            Some(Context::Driver) => {
                self.device.on_read_reply(buf.into(), len);
                self.poll(eventloop);
            }
            Some(_) => {
                trace!("unexpected read reply");
            }
            None => {
                trace!("state not found for read reply on {:?}", handle_id);
            }
        }
    }

    fn on_getattr_reply(
        &mut self,
        eventloop: &mut AppEventLoop,
        ch: &Rc<Channel>,
        buf: BufferMut,
        len: usize,
    ) {
        let handle_id = ch.handle().id();
        match self.contexts.get(&handle_id) {
            Some(Context::Driver) => {
                self.on_mac_read_reply(buf, len);
                self.poll(eventloop);
            }
            Some(_) => {
                trace!("unexpected getattr reply");
            }
            None => {
                trace!("state not found for getattr reply on {:?}", handle_id);
            }
        }
    }

    fn on_write_reply(&mut self, ch: &Rc<Channel>, _buf: Buffer, _len: usize) {
        let handle_id = ch.handle().id();
        match self.contexts.get(&handle_id) {
            Some(Context::Driver) => {
                // Sent a packet.
            }
            Some(_) => {
                trace!("unexpected write reply on {:?}", handle_id);
            }
            None => {
                trace!("state not found for write reply on {:?}", handle_id);
            }
        }
    }

    fn on_peer_closed(&mut self, eventloop: &mut AppEventLoop, ch: &Rc<Channel>) {
        let handle_id = ch.handle().id();
        let mut should_remove_channel = false;
        let mut should_reconnect_driver = false;
        match self.contexts.get_mut(&handle_id) {
            Some(Context::TcpConn { channel_closed, .. }) => {
                *channel_closed = true;
            }
            Some(Context::TcpListener { .. }) => {
                // Nothing to do.
                should_remove_channel = true;
            }
            Some(Context::Driver) => {
                should_remove_channel = true;
                should_reconnect_driver = true;
            }
            Some(Context::ServiceListener) | Some(Context::ControlClient) => {
                // Nothing to do.
                should_remove_channel = true;
            }
            None => {
                trace!("state not found for peer closed on {:?}", handle_id);
                return;
            }
        }

        if should_remove_channel {
            self.contexts.remove(&handle_id);
            eventloop.remove(handle_id);
        }
        if should_reconnect_driver {
            if let Err(error) = self.connect_driver(eventloop) {
                warn!("failed to reconnect ethernet driver: {:?}", error);
            } else {
                trace!("reconnected ethernet driver");
            }
        }
        self.poll(eventloop);
    }

    fn on_timer_expired(&mut self, eventloop: &mut AppEventLoop) {
        trace!("timer expired");
        self.poll(eventloop);
    }
}

#[ftl::main]
fn main() {
    let mut eventloop: AppEventLoop = EventLoop::new().unwrap();
    let mut app = Main::new(&mut eventloop);

    loop {
        match eventloop.wait() {
            Event::Request {
                request: Request::Open(request),
                ..
            } => app.on_open_request(&mut eventloop, request),
            Event::Request {
                request: Request::Read(request),
                ..
            } => app.on_read_request(&mut eventloop, request),
            Event::Request {
                request: Request::Write(request),
                ..
            } => app.on_write_request(&mut eventloop, request),
            Event::Reply {
                reply: Reply::Open { .. },
                ..
            } => {
                warn!("unexpected open reply");
            }
            Event::Reply {
                reply: Reply::Read {
                    client, buf, len, ..
                },
                ..
            } => app.on_read_reply(&mut eventloop, client.channel(), buf, len),
            Event::Reply {
                reply: Reply::Write {
                    client, buf, len, ..
                },
                ..
            } => app.on_write_reply(client.channel(), buf, len),
            Event::Reply {
                reply: Reply::GetAttr {
                    client, buf, len, ..
                },
                ..
            } => app.on_getattr_reply(&mut eventloop, client.channel(), buf, len),
            Event::Reply {
                reply: Reply::SetAttr { client, .. },
                ..
            } => {
                warn!("unexpected setattr reply from {:?}", client.channel());
            }
            Event::Reply {
                reply: Reply::Error { client, error, .. },
                ..
            } => {
                warn!("error reply from {:?}: {:?}", client.channel(), error);
            }
            Event::PeerClosed { ch, .. } => {
                let ch = ch.clone();
                app.on_peer_closed(&mut eventloop, &ch);
            }
            Event::Timer { .. } => app.on_timer_expired(&mut eventloop),
            Event::Irq { interrupt, .. } => {
                warn!("unexpected interrupt: {:?}", interrupt);
            }
            event => {
                warn!("unhandled event: {:?}", event);
            }
        }
    }
}
