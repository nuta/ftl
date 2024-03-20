use ftl_types::channel::SignalSet;
use ftl_types::error::FtlError;

use crate::folio::Folio;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::process::Process;
use crate::scheduler::WaitPoint;
use crate::spinlock::SpinLock;

struct Mutable {
    peer: Option<Handle<Channel>>,
    pending: SignalSet,
}

pub struct Channel {
    buffer: Folio,
    mutable: SpinLock<Mutable>,
    wait_point: WaitPoint,
}

impl Channel {
    pub fn new() -> Result<(Handle<Channel>, Handle<Channel>), FtlError> {
        todo!()
    }

    pub fn notify(&self, signals: SignalSet) -> Result<(), FtlError> {
        let mut ours = self.mutable.lock();
        let peer = match &mut ours.peer {
            Some(peer) => peer,
            None => return Err(FtlError::ClosedByPeer),
        };

        let mut theirs = peer.mutable.lock();
        theirs.pending.add_set(signals);
        peer.wait_point.resume_one();
        Ok(())
    }
}

impl Handleable for Channel {}
