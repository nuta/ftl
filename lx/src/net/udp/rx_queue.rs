use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl_utils::spinlock::SpinLock;

use crate::net::ipv4::Endpoint;
use crate::types::sys::poll::POLLIN;
use crate::wait_queue::WaitQueue;

const MAX_DATAGRAMS: usize = 64;

pub struct Datagram {
    pub src: Endpoint,
    pub payload: Vec<u8>,
}

struct Mutable {
    peer: Option<Endpoint>,
    datagrams: VecDeque<Datagram>,
}

pub struct RxQueue {
    wait_queue: Arc<WaitQueue>,
    mutable: SpinLock<Mutable>,
}

impl RxQueue {
    pub fn new(wait_queue: Arc<WaitQueue>) -> Self {
        Self {
            wait_queue,
            mutable: SpinLock::new(Mutable {
                peer: None,
                datagrams: VecDeque::new(),
            }),
        }
    }

    pub fn handle_rx(&self, src: Endpoint, payload: &[u8]) {
        let mut mutable = self.mutable.lock();
        if !matches!(mutable.peer, Some(peer) if peer == src) {
            return;
        }

        if mutable.datagrams.len() >= MAX_DATAGRAMS {
            return;
        }

        mutable.datagrams.push_back(Datagram {
            src,
            payload: payload.to_vec(),
        });
        drop(mutable);

        let _ = self.wait_queue.notify(Some(POLLIN));
    }

    /// Sets the remote (peer) address for the socket.
    pub fn connect(&self, peer: Endpoint) {
        let mut mutable = self.mutable.lock();
        mutable.peer = Some(peer);

        // Remove datagrams from other sources.
        mutable.datagrams.retain(|datagram| datagram.src == peer);
    }

    pub fn peer(&self) -> Option<Endpoint> {
        self.mutable.lock().peer
    }

    pub fn pop(&self) -> Option<Datagram> {
        self.mutable.lock().datagrams.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.mutable.lock().datagrams.is_empty()
    }
}
