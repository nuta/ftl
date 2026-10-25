use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;

use ftl_driver::dma::DmaBuf;
use ftl_driver::dma::DmaBufWithDrop;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_types::error::ErrorCode;
use ftl_types::time::MonoTime;
use ftl_utils::reserve_slot::ReserveSlot;
use ftl_utils::spinlock::SpinLock;

use crate::device::PollNotifier;
use crate::mux::PhysicalState;
use crate::mux::RxNotify;

const MAX_RX_QUEUE_DEPTH: usize = 128;

struct Route<N> {
    port_start: u32,
    len: u32,
    dst: Weak<Nic<N>>,
}

fn overlaps<N>(routes: &[Route<N>], port_start: u32, len: u32) -> bool {
    for route in routes {
        if port_start < route.port_start + route.len && route.port_start < port_start + len {
            return true;
        }
    }
    false
}

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
}

struct Mutable<N> {
    rx_queue: VecDeque<RxPacket>,
    emitters: VecDeque<N>,
}

pub struct Nic<N> {
    pub(crate) env: &'static dyn Env,
    routes: SpinLock<Vec<Route<N>>>,
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
            routes: SpinLock::new(Vec::new()),
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

    pub fn bind(&self, port_start: u16, len: u16, dst: &Arc<Nic<N>>) -> Result<u16, ErrorCode> {
        let len = len as u32;
        if len == 0 {
            return Err(ErrorCode::InvalidArg);
        }

        let mut routes = self.routes.lock();
        routes.retain(|route| route.dst.strong_count() > 0);

        let mut port_start = port_start as u32;
        if port_start == 0 {
            let mut start = 1024;
            while start + len <= 65536 && overlaps(&routes, start, len) {
                start += len;
            }
            port_start = start;
        }

        if port_start + len > 65536 {
            return Err(ErrorCode::OutOfBounds);
        }

        if overlaps(&routes, port_start, len) {
            return Err(ErrorCode::AlreadyExists);
        }

        routes
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;
        routes.push(Route {
            port_start,
            len,
            dst: Arc::downgrade(dst),
        });
        Ok(port_start as u16)
    }

    pub fn unbind(&self, port_start: u16, len: u16) -> Result<(), ErrorCode> {
        let mut routes = self.routes.lock();
        for i in 0..routes.len() {
            if routes[i].port_start == port_start as u32 && routes[i].len == len as u32 {
                routes.remove(i);
                return Ok(());
            }
        }
        Err(ErrorCode::NotFound)
    }

    pub(crate) fn lookup(&self, port: u16) -> Option<Arc<Nic<N>>> {
        let port = port as u32;
        for route in self.routes.lock().iter() {
            if route.port_start <= port && port < route.port_start + route.len {
                return route.dst.upgrade();
            }
        }
        None
    }

    pub(crate) fn routes_back_to(&self, port: u16, nic: &Nic<N>) -> bool {
        let Some(dst) = self.lookup(port) else {
            return false;
        };
        core::ptr::eq(Arc::as_ptr(&dst), nic)
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
    ) {
        let rx = RxPacket {
            buf: DmaBufWithDrop::new(self.env, buf),
            packet_offset,
            packet_len,
            header_len,
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
