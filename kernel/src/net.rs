use alloc::sync::Arc;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
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

const MAX_TX_PACKET_LEN: usize = 16 * 1024; // 16 KiB
pub const RX_BUFFER_SIZE: usize = 2048;

pub static PNIC: SpinLock<Option<Arc<ftl_netmux::Nic<EventEmitter>>>> = SpinLock::new(None);

pub struct Nic {
    inner: Arc<ftl_netmux::Nic<EventEmitter>>,
}

impl Nic {
    pub fn new(inner: Arc<ftl_netmux::Nic<EventEmitter>>) -> Self {
        Self { inner }
    }
}

impl Handleable for Nic {}

pub fn sys_nic_create_virtual(
    current: &SharedRef<Thread>,
    _ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let nic = Arc::new(ftl_netmux::Nic::new_virtual(&DRIVER_ENV));
    let nic = SharedRef::new(Nic::new(nic))?;
    let handle = Handle::new(nic, HandleRight::READ | HandleRight::WRITE);
    let handle_id = current.hspace().insert(handle)?;

    Ok(SyscallOutput::Done(handle_id.as_usize()))
}

pub fn sys_nic_subscribe(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let nic_id = HandleId::new(ctx.a0);
    let poll_id = HandleId::new(ctx.a1);
    let cookie = ctx.a2;

    let hspace = current.hspace();
    let (nic, poll) =
        hspace.get2::<Nic, Poll>(nic_id, HandleRight::READ, poll_id, HandleRight::WRITE)?;

    let emitter = EventEmitter::new(poll, cookie)?;
    nic.inner.subscribe(emitter)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_nic_bind(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let nic_id = HandleId::new(ctx.a0);
    let port_start = u16::try_from(ctx.a1).map_err(|_| ErrorCode::InvalidArg)?;
    let len = u16::try_from(ctx.a2).map_err(|_| ErrorCode::InvalidArg)?;
    let dst_id = HandleId::new(ctx.a3);

    let hspace = current.hspace();
    let (nic, dst) =
        hspace.get2::<Nic, Nic>(nic_id, HandleRight::WRITE, dst_id, HandleRight::WRITE)?;

    let port_start = nic.inner.bind(port_start, len, &dst.inner)?;
    Ok(SyscallOutput::Done(port_start as usize))
}

pub fn sys_nic_unbind(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let nic_id = HandleId::new(ctx.a0);
    let port_start = u16::try_from(ctx.a1).map_err(|_| ErrorCode::InvalidArg)?;
    let len = u16::try_from(ctx.a2).map_err(|_| ErrorCode::InvalidArg)?;

    let nic = current.hspace().get::<Nic>(nic_id, HandleRight::WRITE)?;
    nic.inner.unbind(port_start, len)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_nic_recv(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let nic_id = HandleId::new(ctx.a0);
    let mut header = USlice::new(UAddr::new(ctx.a2), ctx.a3)?;
    let mut payload = USlice::new(UAddr::new(ctx.a4), ctx.a5)?;

    let nic = current.hspace().get::<Nic>(nic_id, HandleRight::READ)?;
    let payload_len = nic.inner.recv(&mut header, &mut payload)?;
    Ok(SyscallOutput::Done(payload_len))
}

pub fn sys_nic_send(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let nic_id = HandleId::new(ctx.a0);
    let header_len = ctx.a3;
    let payload_len = ctx.a5;

    // Reject too long packets.
    let packet_len = header_len
        .checked_add(payload_len)
        .ok_or(ErrorCode::PacketTooLong)?;
    if packet_len > MAX_TX_PACKET_LEN {
        return Err(ErrorCode::PacketTooLong);
    }

    let mut header = USlice::new(UAddr::new(ctx.a2), header_len)?;
    let mut payload = USlice::new(UAddr::new(ctx.a4), payload_len)?;
    let nic = current.hspace().get::<Nic>(nic_id, HandleRight::WRITE)?;
    nic.inner.send(&mut header, Some(&mut payload))?;
    Ok(SyscallOutput::Done(0))
}

impl ftl_netmux::RxNotify for EventEmitter {
    fn notify(self) -> Result<(), ErrorCode> {
        self.emit()
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
