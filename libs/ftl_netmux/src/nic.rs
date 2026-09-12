use alloc::collections::VecDeque;

use ftl_driver::dma::DmaBuf;
use ftl_driver::dma::DmaBufWithDrop;
use ftl_driver::env::Env;
use ftl_types::error::ErrorCode;
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
    peeked: Option<RxPacket<'a>>,
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
                peeked: None,
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
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;
        mutable.emitters.push_back(notifier);
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

        if mutable.rx_queue.try_reserve(1).is_err() {
            drop(mutable);
            return;
        }

        mutable.rx_queue.push_back(rx);

        // Notify a poll.
        let notifier = mutable.emitters.pop_front();
        drop(mutable);
        if let Some(notifier) = notifier {
            let _ = notifier.notify();
        }
    }

    pub fn peek(&self, writer: &mut dyn BufWriter) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.peeked.is_none() {
            // No peeked packet. Pop the first one from the queue.
            mutable.peeked = mutable.rx_queue.pop_front();
        }

        let rx = mutable.peeked.as_ref().ok_or(ErrorCode::Empty)?;
        let start = rx.packet_offset;
        let end = start + rx.header_len;
        let bytes = &rx.buf.as_slice()[start..end];
        if writer.len() < bytes.len() {
            return Err(ErrorCode::OutOfBounds);
        }
        writer.write(bytes)?;

        Ok(())
    }

    // TODO: How should we handle `peek` and `recv` from multiple threads?
    pub fn recv(&self, writer: &mut dyn BufWriter) -> Result<usize, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(rx) = mutable.peeked.as_ref() else {
            // You must peek first.
            return Err(ErrorCode::Empty);
        };

        let payload_len = rx.packet_len - rx.header_len;
        if writer.len() != payload_len {
            return Err(ErrorCode::OutOfBounds);
        }

        let start = rx.packet_offset + rx.header_len;
        let end = start + payload_len;
        writer.write(&rx.buf.as_slice()[start..end])?;

        // Pop the RX packet from the queue.
        // TODO: Can we simplify this since we've already checked `self.peeked` above?
        let rx = mutable.peeked.take().unwrap();
        drop(mutable);
        drop(rx);

        Ok(payload_len)
    }

    pub fn drop_peeked(&self) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(rx) = mutable.peeked.take() else {
            return Err(ErrorCode::Empty);
        };

        drop(mutable);
        drop(rx);
        Ok(())
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
