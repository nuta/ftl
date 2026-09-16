use alloc::collections::VecDeque;

use ftl_driver::dma::DmaBuf;
use ftl_driver::dma::DmaBufWithDrop;
use ftl_driver::env::Env;
use ftl_types::error::ErrorCode;
use ftl_utils::reserve_slot::ReserveSlot;
use ftl_utils::spinlock::SpinLock;

use crate::mux::RxNotify;

const MAX_RX_QUEUE_DEPTH: usize = 128;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NicId(u32);

impl NicId {
    pub(crate) fn new(id: u32) -> Self {
        Self(id)
    }
}

/// A received packet.
pub(crate) struct RxPacket<'a> {
    buf: DmaBufWithDrop<'a>,
    /// The offset of the IP header in the packet. This is also the length of
    /// the device's header, Ethernet header, and some headroom in `buf`.
    packet_offset: usize,
    /// The length of the packet.
    packet_len: usize,
    /// The total length of the IP and TCP/UDP headers.
    header_len: usize,
}

impl<'a> RxPacket<'a> {
    pub(crate) fn new(
        env: &'a dyn Env,
        buf: DmaBuf,
        packet_offset: usize,
        packet_len: usize,
        header_len: usize,
    ) -> Self {
        Self {
            buf: DmaBufWithDrop::new(env, buf),
            packet_offset,
            packet_len,
            header_len,
        }
    }
}

struct Mutable<'a, N> {
    rx_queue: VecDeque<RxPacket<'a>>,
    emitters: VecDeque<N>,
}

pub struct Nic<'a, N> {
    mutable: SpinLock<Mutable<'a, N>>,
}

impl<'a, N: RxNotify> Nic<'a, N> {
    pub fn new() -> Self {
        Self {
            mutable: SpinLock::new(Mutable {
                rx_queue: VecDeque::new(),
                emitters: VecDeque::new(),
            }),
        }
    }

    pub fn subscribe(&self, notifier: N) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if !mutable.rx_queue.is_empty() {
            // There are pending RX packets. Notify the poll immediately.
            drop(mutable);
            notifier.notify()?;
            return Ok(());
        }

        mutable
            .emitters
            .reserve_slot()
            .map_err(|_| ErrorCode::OutOfMemory)?
            .push_back(notifier);
        Ok(())
    }

    /// Receives a packet from the driver.
    pub fn receive(&self, rx: RxPacket<'a>) {
        let mut mutable = self.mutable.lock();
        if mutable.rx_queue.len() >= MAX_RX_QUEUE_DEPTH {
            // Our RX queue is full. Drop the packet.
            drop(mutable);
            return;
        }

        let Ok(slot) = mutable.rx_queue.reserve_slot() else {
            return;
        };
        slot.push_back(rx);

        // Notify a poll.
        let notifier = mutable.emitters.pop_front();
        drop(mutable);
        if let Some(notifier) = notifier {
            let _ = notifier.notify();
        }
    }

    pub fn recv(
        &self,
        header: &mut dyn BufWriter,
        payload: &mut dyn BufWriter,
    ) -> Result<usize, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let rx = mutable.rx_queue.front().ok_or(ErrorCode::Empty)?;

        let header_start = rx.packet_offset;
        let header_end = header_start + rx.header_len;
        let header_bytes = &rx.buf.as_slice()[header_start..header_end];
        if header.len() < header_bytes.len() {
            return Err(ErrorCode::OutOfBounds);
        }

        let payload_len = rx.packet_len - rx.header_len;
        if payload.len() < payload_len {
            return Err(ErrorCode::OutOfBounds);
        }

        header.write(header_bytes)?;
        let payload_start = header_end;
        let payload_end = payload_start + payload_len;
        payload.write(&rx.buf.as_slice()[payload_start..payload_end])?;

        let rx = mutable.rx_queue.pop_front().unwrap();
        drop(mutable);
        drop(rx);

        Ok(payload_len)
    }
}

pub trait BufReader {
    fn len(&self) -> usize;
    fn read(&mut self, buf: &mut [u8]) -> Result<(), ErrorCode>;
}

pub trait BufWriter {
    fn len(&self) -> usize;
    fn write(&mut self, buf: &[u8]) -> Result<(), ErrorCode>;
}
