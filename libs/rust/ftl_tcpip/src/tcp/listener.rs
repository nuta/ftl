use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::fmt;

use crate::Io;
use crate::socket::AnySocket;
use crate::tcp::TcpConn;
use crate::transport::Port;

struct Mutable<I: Io> {
    pending_accepts: VecDeque<I::TcpAccept>,
}

#[derive(Debug)]
pub enum AcceptError {}

pub struct TcpListener<I: Io> {
    mutable: spin::Mutex<Mutable<I>>,
}

impl<I: Io> TcpListener<I> {
    pub(crate) fn new(local_port: Port) -> Self {
        Self {
            mutable: spin::Mutex::new(Mutable {
                pending_accepts: VecDeque::new(),
            }),
        }
    }

    pub fn accept(&self, req: I::TcpAccept) -> Result<Arc<TcpConn<I>>, AcceptError> {
        todo!()
    }
}

impl<I: Io> AnySocket for TcpListener<I> {}

impl<I: Io> fmt::Debug for TcpListener<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpListener").finish()
    }
}
