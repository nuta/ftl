use ftl_types::error::FtlError;
use ftl_types::signal::SignalBits;

use crate::poll::Poller;
use crate::ref_counted::SharedRef;
use crate::signal::Signal;

pub struct Interrupt {
    signal: SharedRef<Signal>,
}

impl Interrupt {
    pub fn new() -> Result<SharedRef<Interrupt>, FtlError> {
        let signal = Signal::new()?;
        let interrupt = Interrupt {
            signal
        };
        Ok(SharedRef::new(interrupt))
    }

    pub fn add_poller(&self, poller: SharedRef<Poller>) {
        self.signal.add_poller(poller);
    }

    pub fn trigger(&self) -> Result<(), FtlError> {
        self.signal.update(SignalBits::from_raw(1))
    }

    pub fn ack(&self) -> Result<(), FtlError> {
        // TODO:
        Ok(())
    }
}
