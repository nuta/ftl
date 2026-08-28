use crate::dma::DmaBuf;
use crate::env::Env;

#[derive(Debug)]
pub enum Error {
    TxFull,
    HeadroomTooSmall,
    HeadroomTooLarge,
    RxEmpty,
    RxFull,
    BadDevice,
}

#[derive(Debug)]
pub enum Event {
    /// The TX queue has room to send a packet.
    TxAvailable,
    /// The RX queue has a packet available.
    RxAvailable,
}

pub trait Notifier: Send + Sync {
    fn notify(&self, event: Event);
}

pub trait Driver: Send + Sync {
    type Notifier: Notifier;

    /// Reads the MAC address.
    fn mac_address(&self) -> Result<[u8; 6], Error>;

    /// Tries to send a packet.
    ///
    /// If the device's TX queue is full, returns [`Error::TxFull`].
    fn try_send(
        &self,
        env: &dyn Env,
        header_buf: DmaBuf,
        headroom: usize,
        payload_buf: DmaBuf,
    ) -> Result<(), (DmaBuf, DmaBuf, Error)>;

    /// Provides an RX buffer to receive a packet.
    fn provide(&self, env: &dyn Env, buf: DmaBuf) -> Result<(), (Error, DmaBuf)>;

    /// Tries to pop a packet `(buf, headroom, len)` from the RX queue.
    ///
    /// If the queue is empty, returns [`Error::RxEmpty`].
    fn try_receive(&self) -> Result<(DmaBuf, usize, usize), (Error, Option<DmaBuf>)>;

    /// Subscribes to TX events.
    ///
    /// The device driver will notify the notifier when the TX queue has room to send
    /// a packet.
    fn subscribe_tx(&self, notifier: Self::Notifier) -> Result<(), Error>;

    /// Subscribes to RX events.
    ///
    /// The device driver will notify the notifier when a packet is received.
    fn subscribe_rx(&self, notifier: Self::Notifier) -> Result<(), Error>;

    /// Handles an interrupt.
    fn handle_interrupt(&self, env: &dyn Env);
}
