use alloc::collections::VecDeque;
use core::cmp::min;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::poll::EventKind;
use ftl_types::thread::SyscallRegs;
use ftl_utils::reserve_slot::ReserveSlot;
use ftl_utils::ring_buffer::RingBuffer;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::address::USlice;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::poll::EventEmitter;
use crate::poll::Poll;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

const MAX_WRITE_LEN: usize = 512;

static DEVICE: SpinLock<Device> = SpinLock::new(Device::new());

struct Device {
    buf: RingBuffer<u8, 256>,
    emitters: VecDeque<EventEmitter>,
}

impl Device {
    const fn new() -> Self {
        Self {
            buf: RingBuffer::new(),
            emitters: VecDeque::new(),
        }
    }

    fn drain_from_device(&mut self) {
        let mut tmp = [0u8; 16];
        loop {
            let n = crate::arch::console_read(&mut tmp);
            if n == 0 {
                break;
            }

            for &byte in &tmp[..n] {
                let _ = self.buf.try_push(byte);
            }
        }
    }

    fn read(&mut self, uslice: USlice) -> Result<usize, ErrorCode> {
        let mut n = 0;
        while n < uslice.len() {
            // TODO: Implement RingBuffer::pop_slice.
            let Some(byte) = self.buf.pop() else {
                break;
            };

            uslice.subslice(n, 1)?.write(byte)?;
            n += 1;
        }

        Ok(n)
    }

    fn take_emitters(&mut self) -> VecDeque<EventEmitter> {
        core::mem::take(&mut self.emitters)
    }
}

pub struct Console {
    _private: (),
}

impl Console {
    const fn new() -> Self {
        Self { _private: () }
    }
}

impl Handleable for Console {}

pub fn handle_interrupt() {
    let emitters = {
        let mut device = DEVICE.lock();
        device.drain_from_device();
        device.take_emitters()
    };

    for emitter in emitters {
        let _ = emitter.emit(EventKind::PollNotified);
    }
}

pub fn sys_console_open(
    current: &SharedRef<Thread>,
    _ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let console = SharedRef::new(Console::new())?;
    let handle = Handle::new(console, HandleRight::READ | HandleRight::WRITE);
    let handle_id = current.hspace().insert(handle)?;
    Ok(SyscallOutput::Done(handle_id.as_usize()))
}

pub fn sys_console_write(
    _current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let len = min(ctx.a1, MAX_WRITE_LEN);
    if len == 0 {
        return Ok(SyscallOutput::Done(0));
    }

    let mut buf = [0; MAX_WRITE_LEN];
    let slice = &mut buf[..len];
    USlice::new(UAddr::new(ctx.a0), len)?.read_bytes(slice)?;
    crate::arch::console_write(slice);

    Ok(SyscallOutput::Done(len))
}

pub fn sys_console_read(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let id = HandleId::new(ctx.a0);
    let uslice = USlice::new(UAddr::new(ctx.a1), ctx.a2)?;

    let _console = current.hspace().get::<Console>(id, HandleRight::READ)?;

    let mut device = DEVICE.lock();
    let n = device.read(uslice)?;
    Ok(SyscallOutput::Done(n))
}

pub fn sys_console_subscribe(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let console_id = HandleId::new(ctx.a0);
    let poll_id = HandleId::new(ctx.a1);

    let (_console, poll) = current.hspace().get2::<Console, Poll>(
        console_id,
        HandleRight::READ,
        poll_id,
        HandleRight::WRITE,
    )?;

    let mut device = DEVICE.lock();
    let emitter = EventEmitter::new(poll, console_id);
    device
        .emitters
        .reserve_slot()
        .map_err(|_| ErrorCode::OutOfMemory)?
        .push_back(emitter);

    Ok(SyscallOutput::Done(0))
}
