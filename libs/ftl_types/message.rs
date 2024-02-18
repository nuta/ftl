use crate::handle::HandleId;

#[derive(Debug)]
pub enum MessageOrSignal {
    Message(Message),
    Signal(crate::signal::SignalSet),
}

// TODO: IDL
#[derive(Debug)]
pub enum Message {
    Ping(usize),
    Pong(usize),
    NewClient { ch: HandleId },
    ListenIrq { irq: usize },
}
