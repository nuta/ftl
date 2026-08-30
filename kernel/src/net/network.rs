use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::mem::size_of;
use core::slice;

use ftl_driver::dma::DmaBuf;
use ftl_driver::net::Error;
use ftl_types::error::ErrorCode;
use ftl_types::net::IP_PROTOCOL_TCP;
use ftl_types::net::IP_VERSION_4;
use ftl_types::net::NetMatch;
use ftl_types::net::NetRxMeta;
use ftl_types::poll::EventKind;
use ftl_utils::spinlock::SpinLock;

use super::device::Device;
use super::device::Tx;
use super::packet::Ipv4Addr;
use super::packet::Ipv4Inspector;
use crate::address::USlice;
use crate::handle::Handleable;
use crate::net::GLOBAL_ENV;
use crate::poll::EventEmitter;
use crate::shared_ref::SharedRef;

const MAX_RX_QUEUE_DEPTH: usize = 128;
const ETHERNET_HEADER_LEN: usize = 14;
const ARP_PACKET_LEN: usize = 28;
const ETHTYPE_IPV4: u16 = 0x0800;
const ETHTYPE_ARP: u16 = 0x0806;
const OUR_IP: Ipv4Addr = Ipv4Addr::new(0x0a00_020f);
const GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(0x0a00_0202);
#[derive(Clone, Copy)]
struct NetBinding {
    selector: NetMatch,
    cookie: u64,
}

pub struct Router {
    device: SharedRef<Device>,
    networks: Vec<SharedRef<Network>>,
}

impl Router {
    pub fn new(device: SharedRef<Device>) -> Self {
        Self {
            device,
            networks: Vec::new(),
        }
    }

    pub fn add_network(&mut self, network: SharedRef<Network>) -> Result<(), ErrorCode> {
        self.networks
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        self.networks.push(network);
        Ok(())
    }

    fn find_network(
        &self,
        local_ip: Ipv4Addr,
        local_port: u16,
        remote_ip: Ipv4Addr,
        remote_port: u16,
    ) -> Option<(SharedRef<Network>, u64)> {
        let mut best = None;
        for network in &self.networks {
            let Some((specificity, cookie)) =
                network.match_packet(local_ip, local_port, remote_ip, remote_port)
            else {
                continue;
            };
            if best
                .as_ref()
                .is_none_or(|(best_specificity, _, _)| specificity > *best_specificity)
            {
                best = Some((specificity, network.clone(), cookie));
            }
        }
        best.map(|(_, network, cookie)| (network, cookie))
    }

    pub fn device(&self) -> SharedRef<Device> {
        self.device.clone()
    }

    fn recycle(&self, buf: DmaBuf) {
        let driver = self.device.driver();
        if driver.provide(&GLOBAL_ENV, buf).is_err() {
            warn!("net: failed to recycle an RX buffer");
        }
    }

    fn handle_arp(&self, frame: &[u8], src_mac: [u8; 6]) {
        let expected_len = ETHERNET_HEADER_LEN + ARP_PACKET_LEN;
        if frame.len() < expected_len {
            return;
        }

        let operation = u16::from_be_bytes(frame[20..22].try_into().unwrap());
        let sender_ip = u32::from_be_bytes(frame[28..32].try_into().unwrap());
        let target_ip = u32::from_be_bytes(frame[38..42].try_into().unwrap());
        let sender_ip = Ipv4Addr::new(sender_ip);
        let target_ip = Ipv4Addr::new(target_ip);

        self.device.learn_arp(&GLOBAL_ENV, sender_ip, src_mac);
        if operation != 1 || target_ip != OUR_IP {
            return;
        }

        self.device
            .send_arp_reply(&GLOBAL_ENV, src_mac, sender_ip, OUR_IP);
    }

    fn handle_frame(&self, buf: DmaBuf, headroom: usize, frame_len: usize) {
        if frame_len < ETHERNET_HEADER_LEN {
            self.recycle(buf);
            return;
        }

        let frame = &buf.as_slice()[headroom..headroom + frame_len];
        let src_mac = frame[6..12].try_into().unwrap();
        let eth_type = u16::from_be_bytes(frame[12..14].try_into().unwrap());
        if eth_type == ETHTYPE_ARP {
            self.handle_arp(frame, src_mac);
            self.recycle(buf);
            return;
        }
        if eth_type != ETHTYPE_IPV4 {
            self.recycle(buf);
            return;
        }

        let packet_offset = headroom + ETHERNET_HEADER_LEN;
        let packet_len = frame_len - ETHERNET_HEADER_LEN;
        let packet = &buf.as_slice()[packet_offset..packet_offset + packet_len];
        let inspector = match Ipv4Inspector::new_tcp_packet(packet) {
            Ok(inspector) => inspector,
            Err(_) => {
                self.recycle(buf);
                return;
            }
        };
        let local_ip = inspector.dst_ip();
        let local_port = inspector.dst_port();
        let remote_ip = inspector.src_ip();
        let remote_port = inspector.src_port();
        let Some((network, cookie)) =
            self.find_network(local_ip, local_port, remote_ip, remote_port)
        else {
            self.recycle(buf);
            return;
        };

        self.device.learn_arp(&GLOBAL_ENV, remote_ip, src_mac);

        let meta = NetRxMeta {
            cookie,
            packet_len: inspector.packet_len() as u32,
            ip_version: IP_VERSION_4,
            ip_protocol: IP_PROTOCOL_TCP,
            transport_offset: inspector.transport_offset() as u16,
            payload_offset: inspector.payload_offset() as u16,
            reserved: [0; 6],
        };
        let rx = Rx {
            buf,
            packet_offset,
            meta,
        };
        network.enqueue_rx(rx);
    }

    pub fn handle_interrupt(&self) {
        let driver = self.device.driver();
        driver.handle_interrupt(&GLOBAL_ENV);

        loop {
            let rx = match driver.try_receive() {
                Ok(rx) => rx,
                Err((Error::RxEmpty, _)) => break,
                Err((error, buf)) => {
                    warn!("net: failed to receive packet: {:?}", error);
                    if let Some(buf) = buf {
                        self.recycle(buf);
                    }
                    break;
                }
            };
            let (buf, headroom, frame_len) = rx;
            self.handle_frame(buf, headroom, frame_len);
        }
    }
}

struct Rx {
    buf: DmaBuf,
    packet_offset: usize,
    meta: NetRxMeta,
}

struct Mutable {
    rx_queue: VecDeque<Rx>,
    peeked: Option<Rx>,
    emitters: VecDeque<EventEmitter>,
}

pub struct Network {
    device: SharedRef<Device>,
    bindings: SpinLock<Vec<NetBinding>>,
    mutable: SpinLock<Mutable>,
}

impl Network {
    pub fn new(device: SharedRef<Device>) -> Self {
        Self {
            device,
            bindings: SpinLock::new(Vec::new()),
            mutable: SpinLock::new(Mutable {
                rx_queue: VecDeque::new(),
                peeked: None,
                emitters: VecDeque::new(),
            }),
        }
    }

    pub fn subscribe(&self, emitter: EventEmitter) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.peeked.is_some() || !mutable.rx_queue.is_empty() {
            drop(mutable);
            return emitter.emit(EventKind::PollNotified);
        }

        mutable
            .emitters
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        mutable.emitters.push_back(emitter);
        Ok(())
    }

    pub fn bind(&self, selector: NetMatch, cookie: u64) -> Result<(), ErrorCode> {
        if !selector.is_supported() {
            return Err(ErrorCode::INVALID_ARG);
        }

        let mut bindings = self.bindings.lock();
        if bindings.iter().any(|binding| binding.selector == selector) {
            return Err(ErrorCode::ALREADY_EXISTS);
        }
        bindings
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        bindings.push(NetBinding { selector, cookie });
        Ok(())
    }

    pub fn unbind(&self, selector: &NetMatch) -> Result<(), ErrorCode> {
        let mut bindings = self.bindings.lock();
        let index = bindings
            .iter()
            .position(|binding| binding.selector == *selector)
            .ok_or(ErrorCode::INVALID_ARG)?;
        bindings.remove(index);
        Ok(())
    }

    fn match_packet(
        &self,
        local_ip: Ipv4Addr,
        local_port: u16,
        remote_ip: Ipv4Addr,
        remote_port: u16,
    ) -> Option<(u32, u64)> {
        self.bindings
            .lock()
            .iter()
            .filter(|binding| {
                binding.selector.matches(
                    local_ip.as_u32(),
                    local_port,
                    remote_ip.as_u32(),
                    remote_port,
                )
            })
            .max_by_key(|binding| binding.selector.specificity())
            .map(|binding| (binding.selector.specificity(), binding.cookie))
    }

    fn recycle(&self, rx: Rx) {
        let driver = self.device.driver();
        if driver.provide(&GLOBAL_ENV, rx.buf).is_err() {
            warn!("net: failed to recycle an RX buffer");
        }
    }

    pub fn send(&self, header: USlice, payload: USlice) -> Result<(), ErrorCode> {
        let mut tx = Tx::alloc(&GLOBAL_ENV, header.len(), payload.len())?;
        header.read_bytes(tx.ip_header_bytes())?;
        Ipv4Inspector::new_tcp_header(tx.ip_header_bytes()).map_err(|_| ErrorCode::INVALID_ARG)?;
        if let Some(payload_bytes) = tx.payload_bytes() {
            payload.read_bytes(payload_bytes)?;
        }
        self.device.send_ipv4(&GLOBAL_ENV, GATEWAY_IP, tx)
    }

    fn enqueue_rx(&self, rx: Rx) {
        let mut mutable = self.mutable.lock();
        if mutable.rx_queue.len() >= MAX_RX_QUEUE_DEPTH {
            drop(mutable);
            self.recycle(rx);
            return;
        }
        if mutable.rx_queue.try_reserve(1).is_err() {
            drop(mutable);
            self.recycle(rx);
            return;
        }

        mutable.rx_queue.push_back(rx);
        let emitter = mutable.emitters.pop_front();
        drop(mutable);
        if let Some(emitter) = emitter {
            let _ = emitter.emit(EventKind::PollNotified);
        }
    }

    pub fn peek(&self, header: USlice, meta: USlice) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.peeked.is_none() {
            mutable.peeked = mutable.rx_queue.pop_front();
        }

        let Some(rx) = mutable.peeked.as_ref() else {
            return Err(ErrorCode::EMPTY);
        };

        let header_len = rx.meta.payload_offset as usize;
        if header.len() < header_len {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }

        let meta_ptr = &raw const rx.meta;
        let meta_bytes =
            unsafe { slice::from_raw_parts(meta_ptr.cast::<u8>(), size_of::<NetRxMeta>()) };
        meta.write_bytes(meta_bytes)?;
        let start = rx.packet_offset;
        let end = start + header_len;
        header
            .subslice(0, header_len)?
            .write_bytes(&rx.buf.as_slice()[start..end])?;
        Ok(())
    }

    pub fn recv(&self, payload: USlice) -> Result<usize, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(rx) = mutable.peeked.as_ref() else {
            return Err(ErrorCode::EMPTY);
        };
        let payload_len = rx.meta.packet_len as usize - rx.meta.payload_offset as usize;
        if payload.len() != payload_len {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }
        let start = rx.packet_offset + rx.meta.payload_offset as usize;
        let end = start + payload_len;
        payload.write_bytes(&rx.buf.as_slice()[start..end])?;

        let rx = mutable.peeked.take().unwrap();
        drop(mutable);
        self.recycle(rx);
        Ok(payload_len)
    }

    pub fn drop_rx(&self) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(rx) = mutable.peeked.take() else {
            return Err(ErrorCode::EMPTY);
        };
        drop(mutable);
        self.recycle(rx);
        Ok(())
    }
}

impl Handleable for Network {}
