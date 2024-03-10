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
    NetworkTx {
        dst_mac: [u8; 6],
        ether_type: u16,
        payload: [u8; 512],
        payload_len: usize,
    },
    NewClient {
        ch: HandleId,
    },
    ListenIrq {
        irq: usize,
    },
    GetMacAddr,
    MacAddr([u8; 6]),
}
