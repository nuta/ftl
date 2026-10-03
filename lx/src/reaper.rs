use alloc::sync::Arc;

use ftl::poll::Poll;
use ftl::warn;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_utils::fxhash::FxHashMap;
use ftl_utils::spinlock::SpinLock;

use crate::thread::LxThread;

struct Mutable {
    threads: FxHashMap<HandleId, Arc<LxThread>>,
}

/// A thread reaper, which cleans up exited threads.
pub struct Reaper {
    mutable: SpinLock<Mutable>,
}

impl Reaper {
    pub fn new() -> Result<Self, ErrorCode> {
        Ok(Self {
            mutable: SpinLock::new(Mutable {
                threads: FxHashMap::default(),
            }),
        })
    }

    pub fn start_thread(
        &self,
        poll: &Poll,
        thread: Arc<LxThread>,
        entry: usize,
        sp: usize,
    ) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.threads.contains_key(&thread.id()) {
            return Err(ErrorCode::AlreadyExists);
        }

        // Watch for the thread's exit.
        thread.subscribe(poll)?;
        mutable.threads.insert(thread.id(), thread.clone());
        drop(mutable);

        if let Err(err) = thread.start(entry, sp) {
            self.mutable.lock().threads.remove(&thread.id());
            return Err(err);
        }

        Ok(())
    }

    pub fn reap_thread(&self, id: HandleId) {
        let thread = self.mutable.lock().threads.remove(&id);
        let Some(thread) = thread else {
            warn!("reaper: thread {:?} not found", id);
            return;
        };

        if let Err(err) = thread.reap() {
            warn!("reaper: failed to reap a thread: {:?}", err);
        }

        // Free the thread's resources.
        drop(thread);
    }
}
