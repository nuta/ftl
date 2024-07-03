use alloc::vec::Vec;

use ftl_types::error::FtlError;
use ftl_types::poll::PollEvent;
use ftl_types::signal::SignalBits;

use crate::poll::Poller;
use crate::ref_counted::SharedRef;
use crate::sleep::SleepCallbackResult;
use crate::sleep::SleepPoint;
use crate::spinlock::SpinLock;

struct Mutable {
    pollers: Vec<SharedRef<Poller>>,
    pending: SignalBits,
}

pub struct Signal {
    mutable: SpinLock<Mutable>,
    sleep_point: SleepPoint,
}

impl Signal {
    pub fn new() -> Result<SharedRef<Signal>, FtlError> {
        let signal = Signal {
            sleep_point: SleepPoint::new(),
            mutable: SpinLock::new(Mutable {
                pollers: Vec::new(),
                pending: SignalBits::empty(),
            }),
        };

        Ok(SharedRef::new(signal))
    }

    pub fn add_poller(&self, poller: SharedRef<Poller>) {
        let mut mutable = self.mutable.lock();

        if !mutable.pending.is_empty() {
            poller.set_ready(PollEvent::READABLE);
        }

        mutable.pollers.push(poller);
    }

    pub fn update(&self, or_mask: SignalBits) -> Result<(), FtlError> {
        let mut mutable = self.mutable.lock();
        mutable.pending |= or_mask;

        self.sleep_point.wake_all();

        for poller in &mutable.pollers {
            poller.set_ready(PollEvent::READABLE);
        }

        Ok(())
    }

    pub fn wait(&self) -> Result<SignalBits, FtlError> {
        let value = self.sleep_point.sleep_loop(&self.mutable, |mutable| {
            let value = mutable.pending.clear();
            if !value.is_empty() {
                return SleepCallbackResult::Ready(value);
            }

            SleepCallbackResult::Sleep
        });

        Ok(value)
    }
}
