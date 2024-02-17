#[derive(Debug)]
pub enum MessageOrSignal {
    Message(crate::Message),
    Signal(crate::signal::SignalSet),
}
