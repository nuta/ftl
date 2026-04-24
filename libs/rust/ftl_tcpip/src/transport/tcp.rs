use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Drain;
use alloc::vec::Vec;
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
use crate::transport::Port;
use crate::transport::Protocol;
use crate::utils;
use crate::utils::TryPushBack;

#[derive(Debug)]
pub enum Error {}

pub trait Read: Send + Sync {
    fn write(&mut self, buf: &[u8]) -> usize;
    fn complete(self, result: Result<usize, Error>);
}

pub trait Write: Send + Sync {
    fn read(&mut self, buf: &mut [u8]) -> usize;
    fn complete(self, result: Result<usize, Error>);
}

pub trait Accept: Send + Sync {
    fn complete(self, result: Result<(), Error>);
}

pub struct TcpConn<I: Io> {
    pending_writes: VecDeque<I::TcpWrite>,
    pending_reads: VecDeque<I::TcpRead>,
}

impl<I: Io> TcpConn<I> {
    pub(crate) fn new() -> Self {
        Self {
            pending_writes: VecDeque::new(),
            pending_reads: VecDeque::new(),
        }
    }
}

impl<I: Io> AnySocket for TcpConn<I> {}

struct SynReceived {
    remote_ip: IpAddr,
    remote_port: Port,
    init_seq: u32,
    init_ack: u32,
    window_size: u16,
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

fn transmit<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    mut header: TcpHeader,
    remote_ip: IpAddr,
) -> Result<(), TxError> {
    let head_room = size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<TcpHeader>();
    let mut pkt = Packet::new(0, head_room).map_err(TxError::PacketAlloc)?;

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

    fn transmit_locked(
        self: &Arc<Self>,
        inner: &mut TcpListenerInner<I>,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
    ) {
        for syn in &inner.syn_received {
            let mut header = TcpHeader {
                src_port: self.local_port.into(),
                dst_port: syn.remote_port.into(),
                seq: syn.init_seq.into(),
                ack: syn.init_ack.into(),
                window_size: syn.window_size.into(),
                header_len: encode_header_len(size_of::<TcpHeader>()),
                flags: TcpFlags::SYN | TcpFlags::ACK,
                checksum: 0.into(),
                urgent_pointer: 0.into(),
            };

            if let Err(err) = transmit::<I>(devices, routes, header, syn.remote_ip) {
                warn!("TCP: failed to reply to SYN: {:?}", err);
            }
        }
    }

    fn handle_rx(
        self: &Arc<Self>,
        devices: &mut DeviceMap<I::Device>,
        routes: &mut RouteTable,
        pkt: &mut Packet,
        remote_ip: IpAddr,
        remote_port: Port,
        flags: TcpFlags,
        seq: u32,
        _ack: u32,
        window_size: u16,
    ) -> Result<(), RxError> {
        let mut inner = self.inner.lock();

        if flags.contains(TcpFlags::SYN) {
            trace!("TCP: SYN received");
            inner.syn_received.push(SynReceived {
                remote_ip,
                remote_port,
                init_seq: 0,
                init_ack: seq.wrapping_add(1),
                window_size: window_size,
            });

            self.transmit_locked(&mut inner, devices, routes);
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
}

pub(crate) fn handle_rx<I: Io>(
    devices: &mut DeviceMap<I::Device>,
    routes: &mut RouteTable,
    sockets: &mut SocketMap,
    pkt: &mut Packet,
    remote_ip: IpAddr,
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
            // TODO Handle the TCP connection.
            todo!("handle established connection");
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
                        pkt,
                        remote_ip,
                        src_port,
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
