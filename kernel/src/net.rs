use core::mem::MaybeUninit;
use core::mem::size_of;

use ftl_netmux::NetMux;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::net::Rule;
use ftl_types::poll::EventKind;
use ftl_types::thread::SyscallRegs;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::address::USlice;
use crate::driver::DRIVER_ENV;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::poll::EventEmitter;
use crate::poll::Poll;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

const RX_BUFFER_SIZE: usize = 2048;

pub struct Network {
    nic_id: ftl_netmux::NicId,
}

impl Network {
    pub fn new() -> Result<Self, ErrorCode> {
        let nic_id = NET_MUX.lock().create_nic()?;
        Ok(Self { nic_id })
    }

    pub fn subscribe(&self, emitter: EventEmitter) -> Result<(), ErrorCode> {
        NET_MUX.lock().subscribe(self.nic_id, emitter)
    }

    pub fn bind(&self, rule: Rule) -> Result<(), ErrorCode> {
        NET_MUX.lock().bind(self.nic_id, rule)
    }

    pub fn unbind(&self, rule: &Rule) -> Result<(), ErrorCode> {
        NET_MUX.lock().unbind(self.nic_id, rule)
    }

    pub fn send(&self, mut header: USlice, mut payload: USlice) -> Result<(), ErrorCode> {
        NET_MUX
            .lock()
            .send(self.nic_id, &mut header, Some(&mut payload))
    }

    pub fn peek(&self, mut header: USlice) -> Result<(), ErrorCode> {
        NET_MUX.lock().peek(self.nic_id, &mut header)
    }

    pub fn recv(&self, mut payload: USlice) -> Result<usize, ErrorCode> {
        NET_MUX.lock().recv(self.nic_id, &mut payload)
    }

    pub fn drop_peeked(&self) -> Result<(), ErrorCode> {
        NET_MUX.lock().drop_peeked(self.nic_id)
    }
}

impl Drop for Network {
    fn drop(&mut self) {
        NET_MUX.lock().remove_nic(self.nic_id);
    }
}

impl Handleable for Network {}

pub fn sys_net_create(
    current: &SharedRef<Thread>,
    _ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_table = current.isolate().handles();

    let network = Network::new()?;
    let network = SharedRef::new(network)?;
    let handle = Handle::new(network.clone(), HandleRight::READ | HandleRight::WRITE);
    let handle_id = handle_table.lock().insert(handle)?;

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

    network.bind(*rule)?;
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

    network.peek(header)?;
    Ok(SyscallOutput::Done(0))
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

pub static NET_MUX: SpinLock<NetMux<'static, EventEmitter>> =
    SpinLock::new(NetMux::new(&DRIVER_ENV, RX_BUFFER_SIZE));

impl ftl_netmux::RxNotify for EventEmitter {
    fn notify(self) -> Result<(), ErrorCode> {
        self.emit(EventKind::PollNotified)
    }
}

impl ftl_netmux::BufReader for USlice {
    fn len(&self) -> usize {
        USlice::len(*self)
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        self.read_bytes(buf)
    }
}

impl ftl_netmux::BufWriter for USlice {
    fn len(&self) -> usize {
        USlice::len(*self)
    }

    fn write(&mut self, buf: &[u8]) -> Result<(), ErrorCode> {
        let subslice = self.subslice(0, buf.len())?;
        subslice.write_bytes(buf)
    }
}

pub fn init() {
    let device_id = crate::driver::net_device_id();
    NET_MUX.lock().start_dhcp(device_id);
}
