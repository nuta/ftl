use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::mem::size_of;
use core::slice;

use ftl_driver::dma::DmaBuf;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_driver::net::Error;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::net::NET_IPV4;
use ftl_types::net::NET_LISTEN;
use ftl_types::net::NET_TCP;
use ftl_types::net::NetRxInfo;
use ftl_types::poll::EventKind;
use ftl_types::thread::SyscallRegs;
use ftl_utils::spinlock::SpinLock;

use super::device::Device;
use super::device::PollNotifier;
use super::device::Tx;
use super::packet::Ipv4Addr;
use super::packet::Ipv4Inspector;
use crate::address::UAddr;
use crate::address::USlice;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::net::GLOBAL_ENV;
use crate::poll::EventEmitter;
use crate::poll::Poll;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

const MAX_RX_QUEUE_DEPTH: usize = 128;
const RX_BUFFER_SIZE: usize = 2048;
const RX_BUFFER_COUNT: usize = 64;
const ETHERNET_HEADER_LEN: usize = 14;
const ARP_PACKET_LEN: usize = 28;
const ETHTYPE_IPV4: u16 = 0x0800;
const ETHTYPE_ARP: u16 = 0x0806;
const OUR_IP: Ipv4Addr = Ipv4Addr::new(0x0a00_020f);
const GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(0x0a00_0202);
const NETWORK_KIND: usize = NET_IPV4 | NET_TCP | NET_LISTEN;

struct Stack {
    device: SharedRef<Device>,
    networks: Vec<SharedRef<Network>>,
    irq: u8,
}

impl Stack {
    fn new(device: SharedRef<Device>, irq: u8) -> Self {
        Self {
            device,
            networks: Vec::new(),
            irq,
        }
    }

    fn add_network(&mut self, network: SharedRef<Network>) -> Result<(), ErrorCode> {
        self.networks
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        self.networks.push(network);
        Ok(())
    }

    fn find_network(&self, local_ip: Ipv4Addr, local_port: u16) -> Option<SharedRef<Network>> {
        for network in &self.networks {
            if network.matches(local_ip, local_port) {
                return Some(network.clone());
            }
        }
        None
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

    fn inspect_ipv4(&self, packet: &[u8]) -> Option<(NetRxInfo, usize)> {
        let inspector = match Ipv4Inspector::new_tcp_packet(packet) {
            Ok(inspector) => inspector,
            Err(_) => return None,
        };

        let info = NetRxInfo {
            remote_ip: inspector.src_ip().as_u32(),
            local_ip: inspector.dst_ip().as_u32(),
            remote_port: inspector.src_port(),
            local_port: inspector.dst_port(),
            seq: inspector.seq(),
            ack: inspector.ack(),
            payload_len: inspector.payload_len() as u16,
            window_size: inspector.window_size(),
            flags: inspector.flags(),
            reserved: [0; 3],
        };
        Some((info, inspector.payload_offset()))
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
        let Some((info, payload_offset)) = self.inspect_ipv4(packet) else {
            self.recycle(buf);
            return;
        };

        let local_ip = Ipv4Addr::new(info.local_ip);
        let Some(network) = self.find_network(local_ip, info.local_port) else {
            self.recycle(buf);
            return;
        };

        let remote_ip = Ipv4Addr::new(info.remote_ip);
        self.device.learn_arp(&GLOBAL_ENV, remote_ip, src_mac);

        let rx = Rx {
            buf,
            payload_offset: packet_offset + payload_offset,
            info,
        };
        network.enqueue_rx(rx);
    }

    fn handle_interrupt(&self) {
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

static STACK: SpinLock<Option<Stack>> = SpinLock::new(None);

struct ReservedRx {
    token: usize,
    rx: Rx,
}

struct Mutable {
    rx_queue: VecDeque<Rx>,
    reserved: Option<ReservedRx>,
    next_token: usize,
    emitter: Option<EventEmitter>,
}

pub struct Network {
    device: SharedRef<Device>,
    local_ip: Option<Ipv4Addr>,
    local_port: u16,
    mutable: SpinLock<Mutable>,
}

impl Network {
    fn new(device: SharedRef<Device>, local_ip: u32, local_port: u16) -> Self {
        let local_ip = if local_ip == 0 {
            None
        } else {
            Some(Ipv4Addr::new(local_ip))
        };

        Self {
            device,
            local_ip,
            local_port,
            mutable: SpinLock::new(Mutable {
                rx_queue: VecDeque::new(),
                reserved: None,
                next_token: 1,
                emitter: None,
            }),
        }
    }

    fn set_emitter(&self, emitter: EventEmitter) {
        self.mutable.lock().emitter = Some(emitter);
    }

    fn matches(&self, local_ip: Ipv4Addr, local_port: u16) -> bool {
        if self.local_port != local_port {
            return false;
        }

        match self.local_ip {
            Some(our_ip) => our_ip == local_ip,
            None => true,
        }
    }

    fn recycle(&self, rx: Rx) {
        let driver = self.device.driver();
        if driver.provide(&GLOBAL_ENV, rx.buf).is_err() {
            warn!("net: failed to recycle an RX buffer");
        }
    }

    fn send(&self, header: USlice, payload: USlice) -> Result<(), ErrorCode> {
        let mut tx = Tx::alloc(&GLOBAL_ENV, header.len(), payload.len())?;
        header.read_bytes(tx.ip_header_bytes())?;
        Ipv4Inspector::new_tcp_header(tx.ip_header_bytes()).map_err(|_| ErrorCode::INVALID_ARG)?;
        payload.read_bytes(tx.payload_bytes())?;
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
        if let Some(emitter) = &mutable.emitter {
            let _ = emitter.emit(EventKind::PollNotified);
        }
    }

    fn peek(&self) -> Result<(usize, NetRxInfo), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if let Some(reserved) = &mutable.reserved {
            return Ok((reserved.token, reserved.rx.info));
        }

        let Some(rx) = mutable.rx_queue.pop_front() else {
            return Err(ErrorCode::EMPTY);
        };
        let token = mutable.next_token;
        mutable.next_token = mutable.next_token.wrapping_add(1);
        if mutable.next_token == 0 {
            mutable.next_token = 1;
        }

        let info = rx.info;
        mutable.reserved = Some(ReservedRx { token, rx });
        Ok((token, info))
    }

    fn recv(&self, token: usize, payload: USlice) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(reserved) = mutable.reserved.as_ref() else {
            return Err(ErrorCode::EMPTY);
        };
        if reserved.token != token {
            return Err(ErrorCode::INVALID_ARG);
        }

        let payload_len = reserved.rx.info.payload_len as usize;
        if payload.len() != payload_len {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }
        let start = reserved.rx.payload_offset;
        let end = start + payload_len;
        payload.write_bytes(&reserved.rx.buf.as_slice()[start..end])?;

        let reserved = mutable.reserved.take().unwrap();
        drop(mutable);
        self.recycle(reserved.rx);
        Ok(())
    }
}

impl Handleable for Network {}

struct Rx {
    buf: DmaBuf,
    payload_offset: usize,
    info: NetRxInfo,
}

pub fn init() {
    let driver = virtio_net::VirtioNet::<PollNotifier>::init(&GLOBAL_ENV)
        .expect("failed to initialize virtio-net");
    let driver = SharedRef::new(driver).expect("failed to allocate virtio-net driver");
    let driver: SharedRef<dyn Driver<Notifier = PollNotifier>> = driver;

    for _ in 0..RX_BUFFER_COUNT {
        let buf = GLOBAL_ENV
            .alloc_dma(RX_BUFFER_SIZE)
            .expect("failed to allocate virtio-net RX buffer");
        if driver.provide(&GLOBAL_ENV, buf).is_err() {
            panic!("failed to supply virtio-net RX buffer");
        }
    }

    let device = Device::new(driver);
    let device = SharedRef::new(device).expect("failed to allocate network device");
    let pci_device =
        ftl_driver::pci::find_virtio_device(&GLOBAL_ENV, 1).expect("virtio-net disappeared");
    let irq = ftl_driver::pci::get_interrupt_line(&GLOBAL_ENV, &pci_device);

    *STACK.lock() = Some(Stack::new(device, irq));
    crate::arch::interrupt_acquire(irq).expect("failed to enable virtio-net IRQ");
    info!("net: listening for virtio-net IRQ {}", irq);
}

pub fn is_irq(irq: u8) -> bool {
    let stack = STACK.lock();
    match stack.as_ref() {
        Some(stack) => stack.irq == irq,
        None => false,
    }
}

pub fn handle_interrupt() {
    let stack = STACK.lock();
    let stack = stack.as_ref().expect("network stack is not initialized");
    stack.handle_interrupt();
}

pub fn sys_net_acquire(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let poll_id = HandleId::new(ctx.a0);
    let kind = ctx.a1;
    let local_ip = ctx.a2 as u32;
    let local_port = ctx.a3;
    if kind != NETWORK_KIND || local_port > u16::MAX as usize {
        return Err(ErrorCode::INVALID_ARG);
    }

    let handle_table = current.isolate().handles();
    let handles = handle_table.lock();
    let poll = handles.get::<Poll>(poll_id, HandleRight::WRITE)?;
    drop(handles);

    let mut stack_guard = STACK.lock();
    let Some(stack) = stack_guard.as_mut() else {
        return Err(ErrorCode::INVALID_STATE);
    };
    let network = Network::new(stack.device.clone(), local_ip, local_port as u16);
    let network = SharedRef::new(network)?;
    stack.add_network(network.clone())?;
    drop(stack_guard);

    let rights = HandleRight::READ | HandleRight::WRITE;
    let handle = Handle::new(network.clone(), rights);
    let handle_id = handle_table.lock().insert(handle)?;
    let emitter = EventEmitter::new(poll, handle_id);
    network.set_emitter(emitter);
    Ok(SyscallOutput::Done(handle_id.as_usize()))
}

pub fn sys_net_peek(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let info_addr = UAddr::new(ctx.a1);
    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<Network>(network_id, HandleRight::READ)?;

    let (token, info) = network.peek()?;
    let info_ptr = &raw const info;
    let info_len = size_of::<NetRxInfo>();
    let info_bytes = unsafe { slice::from_raw_parts(info_ptr.cast::<u8>(), info_len) };
    USlice::new(info_addr, info_len)?.write_bytes(info_bytes)?;
    Ok(SyscallOutput::Done(token))
}

pub fn sys_net_recv(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let token = ctx.a1;
    let payload = USlice::new(UAddr::new(ctx.a2), ctx.a3)?;
    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<Network>(network_id, HandleRight::READ)?;

    network.recv(token, payload)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_net_send(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let header = USlice::new(UAddr::new(ctx.a2), ctx.a3)?;
    let payload = USlice::new(UAddr::new(ctx.a4), ctx.a5)?;
    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<Network>(network_id, HandleRight::WRITE)?;

    network.send(header, payload)?;
    Ok(SyscallOutput::Done(0))
}
