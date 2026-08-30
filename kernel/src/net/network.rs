use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::mem::size_of;

use ftl_driver::dma::DmaBuf;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::net::FiveTuple;
use ftl_types::net::Rule;
use ftl_types::poll::EventKind;
use ftl_types::thread::SyscallRegs;
use ftl_utils::spinlock::SpinLock;

use super::device::Device;
use super::device::Tx;
use super::packet::Ipv4Addr;
use super::packet::Ipv4Inspector;
use crate::address::UAddr;
use crate::address::USlice;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::net::GLOBAL_ENV;
use crate::net::GLOBAL_ROUTER;
use crate::poll::EventEmitter;
use crate::poll::Poll;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

const MAX_RX_QUEUE_DEPTH: usize = 128;
const GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(0x0a00_0202);

/// A registered rule.
///
/// This struct must not be `Clone`-able to preserve the uniqueness of the
/// cookie.
struct Binding {
    rule: Rule,
    cookie: usize,
}

/// A received packet.
struct RxPacket {
    buf: DmaBuf,
    /// The offset of the IP header in the packet. This is also the length of
    /// the device's header, Ethernet header, and some headroom in `buf`.
    packet_offset: usize,
    /// The length of the packet.
    packet_len: usize,
    /// The length of the IP header.
    header_len: usize,
    /// The cookie of the matching rule.
    ///
    /// FIXME: Remove this field. What if we `net_unbind` concurrently, while
    ///        this cookie is still in the RX queue?
    cookie: usize,
}

struct Mutable {
    rx_queue: VecDeque<RxPacket>,
    peeked: Option<RxPacket>,
    emitters: VecDeque<EventEmitter>,
}

pub struct Network {
    device: SharedRef<Device>,
    bindings: SpinLock<Vec<Binding>>,
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
        if !mutable.rx_queue.is_empty() {
            // There are pending RX packets. Notify the poll immediately.
            drop(mutable);
            emitter.emit(EventKind::PollNotified)?;
            return Ok(());
        }

        mutable
            .emitters
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        mutable.emitters.push_back(emitter);
        Ok(())
    }

    pub fn bind(&self, rule: Rule, cookie: usize) -> Result<(), ErrorCode> {
        // TODO: Do we need a validation for the rule?

        let mut bindings = self.bindings.lock();
        if bindings.iter().any(|binding| binding.rule == rule) {
            return Err(ErrorCode::ALREADY_EXISTS);
        }

        bindings
            .try_reserve(1)
            .map_err(|_| ErrorCode::OUT_OF_MEMORY)?;
        bindings.push(Binding { rule, cookie });
        Ok(())
    }

    pub fn unbind(&self, rule: &Rule) -> Result<usize, ErrorCode> {
        let mut bindings = self.bindings.lock();
        for (index, binding) in bindings.iter().enumerate() {
            if binding.rule == *rule {
                let cookie = binding.cookie;
                bindings.remove(index);
                return Ok(cookie);
            }
        }

        Err(ErrorCode::NOT_FOUND)
    }

    /// Returns a cookie of the matching rule if found.
    pub fn matches(&self, five_tuple: FiveTuple) -> Option<usize> {
        let mut best_specificity = 0;
        let mut best_cookie = None;
        // TODO: Sort the bindings by specificity to avoid iterating through all of them.
        for binding in self.bindings.lock().iter() {
            if let Some(specificity) = binding.rule.matches(five_tuple) {
                if specificity > best_specificity {
                    best_specificity = specificity;
                    best_cookie = Some(binding.cookie);
                }
            }
        }

        best_cookie
    }

    fn recycle_rx_buffer(&self, buf: DmaBuf) {
        let driver = self.device.driver();
        if driver.provide(&GLOBAL_ENV, buf).is_err() {
            warn!("net: failed to recycle an RX buffer");
        }
    }

    /// Sends a packet to the network.
    pub fn send(&self, header: USlice, payload: USlice) -> Result<(), ErrorCode> {
        let mut tx = Tx::alloc(&GLOBAL_ENV, header.len(), payload.len())?;

        // Copy the header from the user.
        header.read_bytes(tx.ip_header_bytes())?;

        // FIXME: Check if the network owns the binding (our IP/port).

        // Validate the IP and TCP headers.
        // TODO: reject IPv6 / UDP
        if let Err(err) = Ipv4Inspector::new_tcp_tx(tx.ip_header_bytes()) {
            // TODO: Return more specific ErrorCode rather than INVALID_ARG
            warn!("invalid network header: {:?}", err);
            return Err(ErrorCode::INVALID_ARG);
        }

        // Copy the payload from the user.
        if let Some(payload_bytes) = tx.payload_bytes() {
            payload.read_bytes(payload_bytes)?;
        }

        // Send the packet.
        self.device.send_ipv4(&GLOBAL_ENV, GATEWAY_IP, tx)
    }

    /// Receives a packet from the driver.
    pub fn receive(
        &self,
        buf: DmaBuf,
        packet_offset: usize,
        packet_len: usize,
        header_len: usize,
        cookie: usize,
    ) {
        let mut mutable = self.mutable.lock();
        if mutable.rx_queue.len() >= MAX_RX_QUEUE_DEPTH {
            // Our RX queue is full. Drop the packet.
            drop(mutable);
            self.recycle_rx_buffer(buf);
            return;
        }

        if mutable.rx_queue.try_reserve(1).is_err() {
            drop(mutable);
            self.recycle_rx_buffer(buf);
            return;
        }

        mutable.rx_queue.push_back(RxPacket {
            buf,
            packet_offset,
            packet_len,
            header_len,
            cookie,
        });

        // Notify a poll.
        let emitter = mutable.emitters.pop_front();
        drop(mutable);
        if let Some(emitter) = emitter {
            let _ = emitter.emit(EventKind::PollNotified);
        }
    }

    pub fn peek(&self, header: USlice) -> Result<usize, ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.peeked.is_none() {
            // No peeked packet. Pop the first one from the queue.
            mutable.peeked = mutable.rx_queue.pop_front();
        }

        let rx = mutable.peeked.as_ref().ok_or(ErrorCode::EMPTY)?;

        if header.len() < rx.header_len {
            // The user-provided buffer is not large enough.
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }

        // Copy the header.
        let start = rx.packet_offset;
        let end = start + rx.header_len;
        header
            .subslice(0, rx.header_len)?
            .write_bytes(&rx.buf.as_slice()[start..end])?;

        Ok(rx.cookie)
    }

    // TODO: How should we handle `peek` and `recv` from multiple threads?
    pub fn recv(&self, payload: USlice) -> Result<usize, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(rx) = mutable.peeked.as_ref() else {
            // You must peek first.
            return Err(ErrorCode::EMPTY);
        };

        let payload_len = rx.packet_len - rx.header_len;
        if payload.len() != payload_len {
            // The user-provided buffer is not large enough.
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }

        // Copy the payload.
        let start = rx.packet_offset + rx.header_len;
        let end = start + payload_len;
        payload.write_bytes(&rx.buf.as_slice()[start..end])?;

        // Pop the RX packet from the queue.
        // TODO: Can we simplify this since we've already checked `self.peeked` above?
        let rx = mutable.peeked.take().unwrap();
        drop(mutable);

        self.recycle_rx_buffer(rx.buf);
        Ok(payload_len)
    }

    pub fn drop_peeked(&self) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(rx) = mutable.peeked.take() else {
            return Err(ErrorCode::EMPTY);
        };

        drop(mutable);
        self.recycle_rx_buffer(rx.buf);
        Ok(())
    }
}

impl Handleable for Network {}

pub fn sys_net_create(
    current: &SharedRef<Thread>,
    _ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_table = current.isolate().handles();

    let device = GLOBAL_ROUTER
        .lock()
        .as_ref()
        .ok_or(ErrorCode::INVALID_STATE)?
        .device()
        .clone();

    let network = SharedRef::new(Network::new(device))?;
    let handle = Handle::new(network.clone(), HandleRight::READ | HandleRight::WRITE);
    let handle_id = handle_table.lock().insert(handle)?;

    GLOBAL_ROUTER
        .lock()
        .as_mut()
        .ok_or(ErrorCode::INVALID_STATE)?
        .add_network(network.clone())?;
    Ok(SyscallOutput::Done(handle_id.as_usize()))
}

pub fn sys_net_subscribe(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let poll_id = HandleId::new(ctx.a1);
    let handle_table = current.isolate().handles();
    let network = handle_table
        .lock()
        .get::<Network>(network_id, HandleRight::READ)?;
    let poll = handle_table
        .lock()
        .get::<Poll>(poll_id, HandleRight::WRITE)?;

    network.subscribe(EventEmitter::new(poll, network_id))?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_net_bind(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let rule_uslice = USlice::new(UAddr::new(ctx.a1), size_of::<Rule>())?;
    let cookie = ctx.a2;

    let mut rule_buf = MaybeUninit::<Rule>::uninit();
    let rule = unsafe { rule_uslice.read_uninit(&mut rule_buf)? };

    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<Network>(network_id, HandleRight::WRITE)?;

    network.bind(*rule, cookie)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_net_unbind(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let rule_uslice = USlice::new(UAddr::new(ctx.a1), size_of::<Rule>())?;
    let mut _buf = MaybeUninit::<Rule>::uninit();
    let rule = unsafe { rule_uslice.read_uninit(&mut _buf)? };

    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<Network>(network_id, HandleRight::WRITE)?;

    let cookie = network.unbind(rule)?;
    Ok(SyscallOutput::Done(cookie))
}

pub fn sys_net_recv(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let payload = USlice::new(UAddr::new(ctx.a1), ctx.a2)?;

    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<Network>(network_id, HandleRight::READ)?;

    let payload_len = network.recv(payload)?;
    Ok(SyscallOutput::Done(payload_len))
}

pub fn sys_net_peek(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let header = USlice::new(UAddr::new(ctx.a1), ctx.a2)?;
    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<Network>(network_id, HandleRight::READ)?;

    let cookie = network.peek(header)?;
    Ok(SyscallOutput::Done(cookie as usize))
}

pub fn sys_net_drop(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let network_id = HandleId::new(ctx.a0);
    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<Network>(network_id, HandleRight::READ)?;

    network.drop_peeked()?;
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
