use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::mem::size_of;

use ftl_driver::dma::DmaBuf;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
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

#[derive(Clone, Copy)]
struct Binding {
    rule: Rule,
    cookie: u64,
}

pub struct Rx {
    pub buf: DmaBuf,
    pub packet_offset: usize,
    pub packet_len: usize,
    pub header_len: usize,
    pub cookie: u64,
}

struct Mutable {
    rx_queue: VecDeque<Rx>,
    peeked: Option<Rx>,
    emitters: VecDeque<EventEmitter>,
}

pub struct Network {
    device: SharedRef<Device>,
    bindings: SpinLock<Vec<Binding>>,
    mutable: SpinLock<Mutable>,
}

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
    let mut rule_buf = MaybeUninit::<Rule>::uninit();
    let rule = unsafe { rule_uslice.read_uninit(&mut rule_buf)? };

    let network = current
        .isolate()
        .handles()
        .lock()
        .get::<Network>(network_id, HandleRight::WRITE)?;

    network.bind(*rule, ctx.a2 as u64)?;
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

    network.unbind(rule)?;
    Ok(SyscallOutput::Done(0))
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

    pub fn bind(&self, rule: Rule, cookie: u64) -> Result<(), ErrorCode> {
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

    pub fn unbind(&self, rule: &Rule) -> Result<(), ErrorCode> {
        let mut bindings = self.bindings.lock();
        for (index, binding) in bindings.iter().enumerate() {
            if binding.rule == *rule {
                bindings.remove(index);
                return Ok(());
            }
        }

        Err(ErrorCode::NOT_FOUND)
    }

    /// Returns a cookie of the matching rule if found.
    pub fn matches(
        &self,
        eth_type: u16,
        ip_proto: u16,
        local_ip: Ipv4Addr,
        local_port: u16,
        remote_ip: Ipv4Addr,
        remote_port: u16,
    ) -> Option<u64> {
        let mut best_specificity = 0;
        let mut best_cookie = 0;
        // TODO: Sort the bindings by specificity to avoid iterating through all of them.
        for binding in self.bindings.lock().iter() {
            let result = binding.rule.matches(
                eth_type,
                ip_proto,
                local_ip.as_u32(),
                local_port,
                remote_ip.as_u32(),
                remote_port,
            );

            if let Some(specificity) = result {
                if specificity > best_specificity {
                    best_specificity = specificity;
                    best_cookie = binding.cookie;
                }
            }
        }

        if best_specificity == 0 {
            None
        } else {
            Some(best_cookie)
        }
    }

    fn recycle_rx_buffer(&self, rx: Rx) {
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

    pub(super) fn enqueue_rx(&self, rx: Rx) {
        let mut mutable = self.mutable.lock();
        if mutable.rx_queue.len() >= MAX_RX_QUEUE_DEPTH {
            drop(mutable);
            self.recycle_rx_buffer(rx);
            return;
        }
        if mutable.rx_queue.try_reserve(1).is_err() {
            drop(mutable);
            self.recycle_rx_buffer(rx);
            return;
        }

        mutable.rx_queue.push_back(rx);
        let emitter = mutable.emitters.pop_front();
        drop(mutable);
        if let Some(emitter) = emitter {
            let _ = emitter.emit(EventKind::PollNotified);
        }
    }

    pub fn peek(&self, header: USlice) -> Result<u64, ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.peeked.is_none() {
            mutable.peeked = mutable.rx_queue.pop_front();
        }
        let rx = mutable.peeked.as_ref().ok_or(ErrorCode::EMPTY)?;

        if header.len() < rx.header_len {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }

        let start = rx.packet_offset;
        let end = start + rx.header_len;
        header
            .subslice(0, rx.header_len)?
            .write_bytes(&rx.buf.as_slice()[start..end])?;
        Ok(rx.cookie)
    }

    pub fn recv(&self, payload: USlice) -> Result<usize, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(rx) = mutable.peeked.as_ref() else {
            return Err(ErrorCode::EMPTY);
        };
        let payload_len = rx.packet_len - rx.header_len;
        if payload.len() != payload_len {
            return Err(ErrorCode::OUT_OF_BOUNDS);
        }
        let start = rx.packet_offset + rx.header_len;
        let end = start + payload_len;
        payload.write_bytes(&rx.buf.as_slice()[start..end])?;

        let rx = mutable.peeked.take().unwrap();
        drop(mutable);
        self.recycle_rx_buffer(rx);
        Ok(payload_len)
    }

    pub fn drop_peeked(&self) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(rx) = mutable.peeked.take() else {
            return Err(ErrorCode::EMPTY);
        };
        drop(mutable);
        self.recycle_rx_buffer(rx);
        Ok(())
    }
}

impl Handleable for Network {}
