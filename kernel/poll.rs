use core::sync::atomic::AtomicU8;
use core::sync::atomic::Ordering;

use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::poll::PollEvent;
use hashbrown::HashMap;

use crate::ref_counted::SharedRef;
use crate::sleep::SleepCallbackResult;
use crate::sleep::SleepPoint;
use crate::spinlock::SpinLock;

pub struct Poller {
    sleep_point: SharedRef<SleepPoint>,
    handle_id: HandleId,
    interests: PollEvent,
    ready: AtomicU8,
}

impl Poller {
    pub fn new(
        sleep_point: SharedRef<SleepPoint>,
        handle_id: HandleId,
        interests: PollEvent,
    ) -> Poller {
        Poller {
            sleep_point,
            handle_id,
            interests,
            ready: AtomicU8::new(0),
        }
    }

    pub fn set_ready(&self, ready: PollEvent) {
        self.ready.fetch_and(ready.as_raw(), Ordering::SeqCst); // TODO: correct ordering
        self.sleep_point.wake_all();
    }
}

pub struct Poll {
    entries: SpinLock<HashMap<HandleId, SharedRef<Poller>>>,
    sleep_point: SharedRef<SleepPoint>,
}

impl Poll {
    pub fn new() -> Poll {
        Poll {
            entries: SpinLock::new(HashMap::new()),
            sleep_point: SharedRef::new(SleepPoint::new()),
        }
    }

    pub fn wait(&self) -> Result<(PollEvent, HandleId), FtlError> {
        self.sleep_point.sleep_loop(&self.entries, |entries| {
            for entry in entries.values() {
                let ready = entry.ready.load(Ordering::SeqCst); // TODO: correct ordering
                if ready != 0 {
                    return SleepCallbackResult::Ready(Ok((
                        PollEvent::from_raw(ready),
                        entry.handle_id,
                    )));
                }
            }

            SleepCallbackResult::Sleep
        })
    }
}
