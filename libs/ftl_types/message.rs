use crate::handle::HandleId;

#[derive(Debug)]
pub enum MessageOrSignal {
    Message(Message),
    Signal(crate::signal::SignalSet),
}

// TODO: IDL
#[derive(Debug)]
pub enum Message {
    Ok,
    Ping(usize),
    Pong(usize),
    NetworkTx(alloc::vec::Vec<u8>),
    NewClient { ch: HandleId },
    ListenIrq { irq: usize },
}
