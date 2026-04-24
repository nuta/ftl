use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;
use core::fmt;
use core::ops::BitOr;
use core::ops::BitOrAssign;

use crate::Io;
use crate::OutOfMemoryError;
use crate::checksum::Checksum;
use crate::device::Device;
use crate::device::DeviceMap;
use crate::endian::Ne;
use crate::ethernet::EthernetHeader;
use crate::ip::IpAddr;
use crate::ip::ipv4;
use crate::ip::ipv4::Ipv4Addr;
use crate::ip::ipv4::Ipv4Header;
use crate::packet;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::route::RouteTable;
use crate::socket::ActiveKey;
use crate::socket::AnySocket;
use crate::socket::Endpoint;
use crate::socket::ListenerKey;
use crate::socket::SocketMap;
use crate::socket::TryInsertError;
use crate::transport::Port;
use crate::transport::Protocol;
use crate::utils::TryPushBack;

const INITIAL_SEND_SEQ: u32 = 0;
const DEFAULT_RCV_WND: u16 = u16::MAX;

fn saturating_usize_to_u16(value: usize) -> u16 {
    min(value, u16::MAX as usize) as u16
}

#[derive(Debug)]
pub enum Error {}

pub trait Read: Send + Sync {
    fn write(&mut self, buf: &[u8]) -> usize;
    fn complete(self, result: Result<usize, Error>);
}

pub trait Write: Send + Sync {
    fn len(&self) -> usize;
    fn read(&mut self, buf: &mut [u8]) -> usize;
    fn complete(self, result: Result<usize, Error>);
}

pub trait Accept: Send + Sync {
    fn complete(self, result: Result<(), Error>);
}

#[derive(Debug)]
enum State {
    Established,
    FinWait1,
    FinWait2,
    Closing,
}

struct TcpConnMutable<I: Io> {
    state: State,
    /// Sequence number of the first byte not yet acknowledged by the peer.
    snd_una: u32,
    /// Sequence number of the next byte to send.
    snd_nxt: u32,
    /// Peer's receive window size: how much we can send. Fullfilled when the
    /// peer sends an ACK.
    snd_wnd: u16,
    /// Sequence number of the next byte we expect to receive.
    rcv_nxt: u32,
    /// Our receive window size. How much RX buffer space we have.
    rcv_wnd: u16,
    /// The receive buffer.
    rx: Vec<u8>,
    /// The send buffer.
    tx: Vec<u8>,
    pending_writes: VecDeque<I::TcpWrite>,
    pending_reads: VecDeque<I::TcpRead>,
}

impl<I: Io> TcpConnMutable<I> {
    fn receive_bytes(&mut self, buf: &[u8]) {
        // TODO: buffer size
        self.rx.extend_from_slice(buf);
        self.rcv_nxt = self.rcv_nxt.wrapping_add(buf.len() as u32);
        self.rcv_wnd = self
            .rcv_wnd
            .saturating_sub(saturating_usize_to_u16(buf.len()));

        info!("TCP: received {} bytes", buf.len());

        if let Some(mut req) = self.pending_reads.pop_front() {
            let len = req.write(self.rx.as_slice());
            req.complete(Ok(len));
            self.rx.drain(..len);
            self.rcv_wnd = self.rcv_wnd.saturating_add(saturating_usize_to_u16(len));
        }
    }

    fn update_send_window(&mut self, ack: u32, window_size: u16) {
        self.snd_wnd = window_size;

        let acked_bytes = ack.wrapping_sub(self.snd_una);
        let in_flight = self.snd_nxt.wrapping_sub(self.snd_una);
        if acked_bytes == 0 || acked_bytes > in_flight {
            return;
        }

        self.snd_una = ack;
        let drain_len = min(acked_bytes as usize, self.tx.len());
        self.tx.drain(..drain_len);
    }
}

pub struct TcpConn<I: Io> {
    local_port: Port,
    remote: Endpoint,
    mutable: spin::Mutex<TcpConnMutable<I>>,
}

impl<I: Io> TcpConn<I> {
    fn new(
        local_port: Port,
        remote: Endpoint,
        iss: u32,
        rcv_nxt: u32,
        snd_wnd: u16,
        rcv_wnd: u16,
        state: State,
    ) -> Self {
        let snd_nxt = iss.wrapping_add(1); // +1 for the SYN packet
        Self {
            local_port,
            remote,
            mutable: spin::Mutex::new(TcpConnMutable {
                state,
                snd_una: snd_nxt,
                snd_nxt,
                snd_wnd,
                rcv_nxt,
                rcv_wnd,
                rx: Vec::new(),
                tx: Vec::new(),
                pending_writes: VecDeque::new(),
                pending_reads: VecDeque::new(),
            }),
        }
    }

    fn receive_bytes(&self, buf: &[u8]) {
        let mut mutable = self.mutable.lock();
        mutable.receive_bytes(buf);
    }

    fn handle_rx(
        &self,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        pkt: &mut Packet,
        flags: TcpFlags,
        seq: u32,
        ack: u32,
        window_size: u16,
    ) {
        let mut mutable = self.mutable.lock();
        if flags.contains(TcpFlags::ACK) {
            mutable.update_send_window(ack, window_size);
        } else {
            mutable.snd_wnd = window_size;
        }

        let payload = pkt.slice();
        let mut should_ack = false;
        if !payload.is_empty() {
            if seq == mutable.rcv_nxt {
                mutable.receive_bytes(payload);
            } else {
                trace!(
                    "TCP: out-of-order data: seq={}, rcv_nxt={}",
                    seq, mutable.rcv_nxt
                );
            }

            should_ack = true;
        }

        let mut received_fin = false;
        if flags.contains(TcpFlags::FIN) {
            let fin_seq = seq.wrapping_add(payload.len() as u32);
            if fin_seq == mutable.rcv_nxt {
                mutable.rcv_nxt = mutable.rcv_nxt.wrapping_add(1);
                received_fin = true;
            }

            should_ack = true;
        }

        if matches!(mutable.state, State::FinWait1) && mutable.snd_una == mutable.snd_nxt {
            trace!("TCP: FIN acknowledged");
            mutable.state = State::FinWait2;
        }

        if received_fin && matches!(mutable.state, State::FinWait1 | State::FinWait2) {
            trace!("TCP: FIN received");
            mutable.state = State::Closing;
        }

        if should_ack {
            let header = TcpHeader {
                src_port: self.local_port.into(),
                dst_port: self.remote.port.into(),
                seq: mutable.snd_nxt.into(),
                ack: mutable.rcv_nxt.into(),
                window_size: mutable.rcv_wnd.into(),
                header_len: encode_header_len(size_of::<TcpHeader>()),
                flags: TcpFlags::ACK,
                checksum: 0.into(),
                urgent_pointer: 0.into(),
            };

            if let Err(err) = transmit_segment::<I>(devices, routes, header, self.remote.addr, &[])
            {
                warn!("TCP: failed to send ACK: {:?}", err);
            }
        }

        self.poll_locked(devices, routes, &mut mutable);
    }

    pub fn write(
        &self,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        mut req: I::TcpWrite,
    ) {
        let mut mutable = self.mutable.lock();
        let mut tmp = alloc::vec![0; req.len()];
        let read_len = req.read(&mut tmp);
        mutable.tx.extend_from_slice(&tmp[..read_len]);
        req.complete(Ok(read_len));
        self.poll_locked(devices, routes, &mut mutable);
    }

    pub fn close(&self, devices: &mut DeviceMap<I::Device>, routes: &mut RouteTable) {
        let mut mutable = self.mutable.lock();
        mutable.state = State::FinWait1;

        trace!("TCP: sending FIN");
        let header = TcpHeader {
            src_port: self.local_port.into(),
            dst_port: self.remote.port.into(),
            seq: mutable.snd_nxt.into(),
            ack: mutable.rcv_nxt.into(),
            window_size: mutable.rcv_wnd.into(),
            header_len: encode_header_len(size_of::<TcpHeader>()),
            flags: TcpFlags::FIN | TcpFlags::ACK,
            checksum: 0.into(),
            urgent_pointer: 0.into(),
        };

        match transmit_segment::<I>(devices, routes, header, self.remote.addr, &[]) {
            Ok(()) => {
                mutable.snd_nxt = mutable.snd_nxt.wrapping_add(1);
            }
            Err(err) => {
                warn!("TCP: failed to send FIN: {:?}", err);
            }
        }
    }

    fn poll_locked(
        &self,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        mutable: &mut TcpConnMutable<I>,
    ) {
        match &mut mutable.state {
            State::Established => {
                // Bytes between snd_una and snd_nxt have already been sent but
                // are still waiting for an ACK, so they still consume the peer
                // receive window.
                let in_flight = mutable.snd_nxt.wrapping_sub(mutable.snd_una);
                let usable_window = (mutable.snd_wnd as u32).saturating_sub(in_flight);

                // The TX buffer starts at snd_una. Skip bytes that are already
                // in flight and limit new payload to the remaining usable
                // window.
                let unsent_offset = min(in_flight as usize, mutable.tx.len());
                let unsent_bytes = mutable.tx.len() - unsent_offset;
                let sendable_bytes = min(usable_window as usize, unsent_bytes);

                if sendable_bytes > 0 {
                    let payload = &mutable.tx[unsent_offset..unsent_offset + sendable_bytes];
                    let header = TcpHeader {
                        src_port: self.local_port.into(),
                        dst_port: self.remote.port.into(),
                        seq: mutable.snd_nxt.into(),
                        ack: mutable.rcv_nxt.into(),
                        window_size: mutable.rcv_wnd.into(),
                        header_len: encode_header_len(size_of::<TcpHeader>()),
                        flags: TcpFlags::ACK | TcpFlags::PSH,
                        checksum: 0.into(),
                        urgent_pointer: 0.into(),
                    };

                    trace!("TCP: sending {} bytes", payload.len());
                    if let Err(err) =
                        transmit_segment::<I>(devices, routes, header, self.remote.addr, payload)
                    {
                        warn!("TCP: failed to send data: {:?}", err);
                    } else {
                        mutable.snd_nxt = mutable.snd_nxt.wrapping_add(sendable_bytes as u32);
                    }
                }
            }
            State::FinWait1 | State::FinWait2 | State::Closing => {}
        }
    }
}

impl<I: Io> AnySocket for TcpConn<I> {}

struct SynReceived {
    remote_ip: IpAddr,
    remote_port: Port,
    local_iss: u32,
    remote_rcv_nxt: u32,
}

struct TcpListenerInner<I: Io> {
    pending_accepts: VecDeque<I::TcpAccept>,
    syn_received: Vec<SynReceived>,
}

#[derive(Debug)]
enum TxError {
    PacketAlloc(packet::AllocError),
    PacketWrite(packet::ReserveError),
    Ipv4Tx(ipv4::TxError),
    NoRoute,
    NoDevice,
}

fn transmit_segment<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    mut header: TcpHeader,
    remote_ip: IpAddr,
    payload: &[u8],
) -> Result<(), TxError> {
    let head_room = size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<TcpHeader>();
    let mut pkt = Packet::new(payload.len(), head_room).map_err(TxError::PacketAlloc)?;
    pkt.write_back_bytes(payload)
        .map_err(TxError::PacketWrite)?;

    match remote_ip {
        IpAddr::V4(remote_ipv4) => {
            let Some(route) = routes.lookup_by_dest(remote_ip) else {
                return Err(TxError::NoRoute);
            };

            let Some(device) = devices.get_mut(route.device_id()) else {
                return Err(TxError::NoDevice);
            };

            header.checksum = header
                .compute_checksum(route.ipv4_addr(), remote_ipv4, pkt.slice())
                .into();

            pkt.write_front(header).map_err(TxError::PacketWrite)?;

            ipv4::transmit::<I>(
                device,
                route,
                &mut pkt,
                route.ipv4_addr(),
                remote_ipv4,
                Protocol::Tcp,
            )
            .map_err(TxError::Ipv4Tx)?;
        }
    }

    Ok(())
}

pub struct TcpListener<I: Io> {
    local_port: Port,
    inner: spin::Mutex<TcpListenerInner<I>>,
}

impl<I: Io> TcpListener<I> {
    pub(crate) fn new(local_port: Port) -> Self {
        Self {
            local_port,
            inner: spin::Mutex::new(TcpListenerInner {
                pending_accepts: VecDeque::new(),
                syn_received: Vec::new(),
            }),
        }
    }

    pub fn accept(&mut self, req: I::TcpAccept) -> Result<(), OutOfMemoryError> {
        self.inner.lock().pending_accepts.try_push_back(req)?;
        Ok(())
    }

    fn handle_rx(
        self: &Arc<Self>,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        sockets: &mut SocketMap,
        pkt: &mut Packet,
        remote_ip: IpAddr,
        remote_port: Port,
        local_ip: IpAddr,
        flags: TcpFlags,
        seq: u32,
        ack: u32,
        window_size: u16,
    ) -> Result<(), RxError> {
        let mut inner = self.inner.lock();

        if flags.contains(TcpFlags::SYN) {
            trace!("TCP: SYN received");
            let syn = SynReceived {
                remote_ip,
                remote_port,
                local_iss: INITIAL_SEND_SEQ,
                remote_rcv_nxt: seq.wrapping_add(1),
            };

            let header = TcpHeader {
                src_port: self.local_port.into(),
                dst_port: syn.remote_port.into(),
                seq: syn.local_iss.into(),
                ack: syn.remote_rcv_nxt.into(),
                window_size: DEFAULT_RCV_WND.into(),
                header_len: encode_header_len(size_of::<TcpHeader>()),
                flags: TcpFlags::SYN | TcpFlags::ACK,
                checksum: 0.into(),
                urgent_pointer: 0.into(),
            };

            inner.syn_received.push(syn);
            if let Err(err) = transmit_segment::<I>(devices, routes, header, remote_ip, &[]) {
                warn!("TCP: failed to reply to SYN: {:?}", err);
            }
        } else if flags.contains(TcpFlags::ACK) {
            trace!("TCP: ACK received");
            let Some((syn_index, syn)) = inner
                .syn_received
                .iter()
                .enumerate()
                .find(|(_, syn)| remote_ip == syn.remote_ip && remote_port == syn.remote_port)
            else {
                return Err(RxError::BadAckToListener);
            };

            let expected_ack = syn.local_iss.wrapping_add(1);
            if ack != expected_ack {
                return Err(RxError::BadAckNumber {
                    expected: expected_ack,
                    actual: ack,
                });
            }

            let syn = inner.syn_received.remove(syn_index);
            let remote = Endpoint {
                addr: remote_ip,
                port: remote_port,
            };
            let conn = TcpConn::<I>::new(
                self.local_port,
                remote,
                syn.local_iss,
                syn.remote_rcv_nxt,
                window_size,
                DEFAULT_RCV_WND,
                State::Established,
            );

            let key = ActiveKey {
                remote,
                local: Endpoint {
                    addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    port: self.local_port,
                },
                protocol: Protocol::Tcp,
            };

            let payload = pkt.slice();
            if !payload.is_empty() {
                conn.receive_bytes(payload);
            }

            sockets
                .insert_active(key, Arc::new(conn))
                .map_err(RxError::InsertActive)?;
        } else {
            trace!("TCP: unknown flags: {:?}", flags);
        }

        Ok(())
    }
}

impl<I: Io> AnySocket for TcpListener<I> {}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct TcpFlags(u8);

impl TcpFlags {
    pub const FIN: Self = Self(1 << 0);
    pub const SYN: Self = Self(1 << 1);
    pub const RST: Self = Self(1 << 2);
    pub const PSH: Self = Self(1 << 3);
    pub const ACK: Self = Self(1 << 4);

    pub fn contains(&self, flag: TcpFlags) -> bool {
        self.0 & flag.0 != 0
    }
}

impl BitOr<TcpFlags> for TcpFlags {
    type Output = TcpFlags;

    fn bitor(self, rhs: TcpFlags) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign<TcpFlags> for TcpFlags {
    fn bitor_assign(&mut self, rhs: TcpFlags) {
        self.0 |= rhs.0;
    }
}

impl fmt::Debug for TcpFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (value, name) in [
            (TcpFlags::FIN, "FIN"),
            (TcpFlags::SYN, "SYN"),
            (TcpFlags::RST, "RST"),
            (TcpFlags::PSH, "PSH"),
            (TcpFlags::ACK, "ACK"),
        ] {
            if self.0 & value.0 != 0 {
                if !first {
                    write!(f, "|")?;
                }

                write!(f, "{name}")?;
                first = false;
            }
        }

        Ok(())
    }
}

#[repr(C, packed)]
struct TcpHeader {
    src_port: Ne<u16>,
    dst_port: Ne<u16>,
    seq: Ne<u32>,
    ack: Ne<u32>,
    header_len: u8,
    flags: TcpFlags,
    window_size: Ne<u16>,
    checksum: Ne<u16>,
    urgent_pointer: Ne<u16>,
}

impl WriteableToPacket for TcpHeader {}

fn encode_header_len(len: usize) -> u8 {
    debug_assert_eq!(len % 4, 0);
    debug_assert!(len / 4 <= 0x0f);
    ((len / 4) as u8) << 4
}

impl TcpHeader {
    fn compute_checksum(&self, src_ip: Ipv4Addr, dst_ip: Ipv4Addr, payload: &[u8]) -> u16 {
        let tcp_len = size_of::<Self>() + payload.len();
        debug_assert!(tcp_len <= u16::MAX as usize);

        let mut checksum = Checksum::new();
        checksum.supply_u32(src_ip.as_u32());
        checksum.supply_u32(dst_ip.as_u32());
        checksum.supply_u16(Protocol::Tcp as u16);
        checksum.supply_u16(tcp_len as u16);
        checksum.supply_u16(self.src_port.into());
        checksum.supply_u16(self.dst_port.into());
        checksum.supply_u32(self.seq.into());
        checksum.supply_u32(self.ack.into());
        checksum.supply_u16(((self.header_len as u16) << 8) | self.flags.0 as u16);
        checksum.supply_u16(self.window_size.into());
        checksum.supply_u16(0);
        checksum.supply_u16(self.urgent_pointer.into());
        checksum.supply_bytes(payload);
        checksum.finish()
    }
}

#[derive(Debug)]
pub(crate) enum RxError {
    PacketRead(packet::ReserveError),
    BadAckToListener,
    BadAckNumber { expected: u32, actual: u32 },
    InsertActive(TryInsertError),
}

pub(crate) fn handle_rx<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    sockets: &mut SocketMap,
    pkt: &mut Packet,
    remote_ip: IpAddr,
    local_ip: IpAddr,
) -> Result<(), RxError> {
    let header = pkt.read::<TcpHeader>().map_err(RxError::PacketRead)?;
    let src_port = Port::from(header.src_port);
    let dst_port = Port::from(header.dst_port);
    let flags = header.flags;
    let seq = header.seq.into();
    let ack = header.ack.into();
    let window_size = header.window_size.into();

    trace!(
        "TCP packet [flags: {:?}] src_port: {}, dst_port: {}, {:?}",
        flags,
        src_port,
        dst_port,
        core::str::from_utf8(pkt.slice()).unwrap_or("(invalid UTF-8)"),
    );

    let key = ActiveKey {
        local: Endpoint {
            addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: dst_port,
        },
        protocol: Protocol::Tcp,
        remote: Endpoint {
            addr: remote_ip,
            port: src_port,
        },
    };

    match sockets.get_active::<TcpConn<I>>(&key) {
        Some(conn) => {
            conn.handle_rx(devices, routes, pkt, flags, seq, ack, window_size);
        }
        None => {
            let key = ListenerKey {
                local: Endpoint {
                    addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    port: dst_port,
                },
                protocol: Protocol::Tcp,
            };

            match sockets.get_listener::<TcpListener<I>>(&key) {
                Some(listener) => {
                    listener.handle_rx(
                        devices,
                        routes,
                        sockets,
                        pkt,
                        remote_ip,
                        src_port,
                        local_ip,
                        flags,
                        seq,
                        ack,
                        window_size,
                    );
                }
                None => {
                    trace!("TCP: no connection or listener found");
                    // TODO: Send an RST packet.
                }
            }
        }
    }

    Ok(())
}
