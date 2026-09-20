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
use crate::poll::EventEmitter;
use crate::poll::Poll;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

const MAX_WRITE_LEN: usize = 512;

static CONSOLE: SpinLock<Console> = SpinLock::new(Console::new());

struct Console {
    buf: RingBuffer<u8, 256>,
    emitters: VecDeque<EventEmitter>,
}

impl Console {
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

pub fn handle_interrupt() {
    let emitters = {
        let mut console = CONSOLE.lock();
        console.drain_from_device();
        console.take_emitters()
    };

    for emitter in emitters {
        let _ = emitter.emit(EventKind::PollNotified);
    }
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
    _current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let uslice = USlice::new(UAddr::new(ctx.a0), ctx.a1)?;

    let mut console = CONSOLE.lock();
    let mut n = console.read(uslice)?;
    Ok(SyscallOutput::Done(n))
}

pub fn sys_console_subscribe(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let poll_id = HandleId::new(ctx.a0);
    let poll = current.hspace().get::<Poll>(poll_id, HandleRight::WRITE)?;
    let emitter = EventEmitter::new(poll, poll_id);

    let mut console = CONSOLE.lock();
    console
        .emitters
        .reserve_slot()
        .map_err(|_| ErrorCode::OutOfMemory)?
        .push_back(emitter);
    Ok(SyscallOutput::Done(0))
}
