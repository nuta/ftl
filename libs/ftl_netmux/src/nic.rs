use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::ptr;

use ftl_driver::dma::DmaBuf;
use ftl_driver::dma::DmaBufWithDrop;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_types::error::ErrorCode;
use ftl_types::nic::RouteRule;
use ftl_types::time::MonoTime;
use ftl_utils::reserve_slot::ReserveSlot;
use ftl_utils::spinlock::SpinLock;

use crate::device::PollNotifier;
use crate::packet::ipv4::Ipv4Addr;
use crate::physical::PhysicalState;
use crate::physical::RxNotify;
use crate::route::RouteTable;

const MAX_RX_QUEUE_DEPTH: usize = 128;

/// A received packet.
struct RxPacket {
    buf: DmaBufWithDrop<'static>,
    /// The offset of the IP header in the packet. This is also the length of
    /// the device's header, Ethernet header, and some headroom in `buf`.
    packet_offset: usize,
    /// The length of the packet.
    packet_len: usize,
    /// The total length of the IP and TCP/UDP headers.
    header_len: usize,
    payload_buf: Option<DmaBufWithDrop<'static>>,
}

struct Mutable<N> {
    rx_queue: VecDeque<RxPacket>,
    emitters: VecDeque<N>,
}

pub struct Nic<N> {
    pub(crate) env: &'static dyn Env,
    routes: SpinLock<RouteTable<N>>,
    mutable: SpinLock<Mutable<N>>,
    pub(crate) physical: Option<SpinLock<PhysicalState<'static>>>,
}

impl<N: RxNotify> Nic<N> {
    fn new(env: &'static dyn Env, physical: Option<PhysicalState<'static>>) -> Self {
        let physical = match physical {
            Some(physical) => Some(SpinLock::new(physical)),
            None => None,
        };

        Self {
            env,
            routes: SpinLock::new(RouteTable::new()),
            mutable: SpinLock::new(Mutable {
                rx_queue: VecDeque::new(),
                emitters: VecDeque::new(),
            }),
            physical,
        }
    }

    pub fn new_virtual(env: &'static dyn Env) -> Self {
        Self::new(env, None)
    }

    pub fn new_physical(
        env: &'static dyn Env,
        rx_buffer_size: usize,
        driver: Box<dyn Driver<Notifier = PollNotifier>>,
        irq: u8,
    ) -> Self {
        let physical = PhysicalState::new(env, rx_buffer_size, driver, irq);
        Self::new(env, Some(physical))
    }

    pub fn start_dhcp(&self, now: MonoTime) {
        if let Some(physical) = &self.physical {
            physical.lock().start_dhcp(now);
        }
    }

    pub fn poll(&self, irq: Option<u8>, now: MonoTime) {
        if let Some(physical) = &self.physical {
            physical.lock().poll(self, irq, now);
        }
    }

    pub fn bind(&self, rule: &RouteRule, dst: &Arc<Nic<N>>) -> Result<(), ErrorCode> {
        self.routes.lock().bind(rule, dst)
    }

    pub fn unbind(&self, rule: &RouteRule) -> Result<(), ErrorCode> {
        self.routes.lock().unbind(rule)
    }

    /// Returns the destination NIC and source IP address to overwrite.
    pub(crate) fn lookup_route(
        &self,
        protocols: u8,
        dst_ip: Ipv4Addr,
        dst_port: u16,
    ) -> Option<(Arc<Nic<N>>, Option<Ipv4Addr>)> {
        self.routes.lock().lookup(protocols, dst_ip, dst_port)
    }

    /// Reverse path filtering: check if the destination NIC has a route to
    /// this NIC, to prevent IP spoofing.
    pub(crate) fn has_route_back(
        &self,
        peer: &Nic<N>,
        protocols: u8,
        src_ip: Ipv4Addr,
        src_port: u16,
    ) -> bool {
        let Some((back, _)) = peer.routes.lock().lookup(protocols, src_ip, src_port) else {
            return false;
        };

        ptr::eq(Arc::as_ptr(&back), self)
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
    pub(crate) fn receive(
        &self,
        buf: DmaBuf,
        packet_offset: usize,
        packet_len: usize,
        header_len: usize,
        payload_buf: Option<DmaBuf>,
    ) {
        let rx = RxPacket {
            buf: DmaBufWithDrop::new(self.env, buf),
            packet_offset,
            packet_len,
            header_len,
            payload_buf: payload_buf.map(|buf| DmaBufWithDrop::new(self.env, buf)),
        };

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

        let payload_len = rx.packet_len - rx.header_len;
        if header.len() < rx.header_len || payload.len() < payload_len {
            // Drop the too short/long packet.
            let rx = mutable.rx_queue.pop_front();
            drop(mutable);
            drop(rx);

            return Err(ErrorCode::OutOfBounds);
        }

        // Copy the header.
        let header_start = rx.packet_offset;
        let header_end = header_start + rx.header_len;
        let header_bytes = &rx.buf.as_slice()[header_start..header_end];
        header.write(header_bytes)?;

        // Copy the payload.
        let payload_bytes = match &rx.payload_buf {
            Some(payload_buf) => &payload_buf.as_slice()[..payload_len],
            None => &rx.buf.as_slice()[header_end..header_end + payload_len],
        };
        payload.write(payload_bytes)?;

        // Remove the copied packet from the queue.
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
