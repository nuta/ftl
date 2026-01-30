use alloc::rc::Rc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use ftl_types::channel::TxId;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::buffer::Buffer;
use crate::buffer::BufferMut;
use crate::channel::Channel;
use crate::channel::Reply;
use crate::channel::SendError;
use crate::sink::Event;
use crate::sink::Sink;

pub struct OpenRequest {
    ch: Rc<Channel>,
    txid: TxId,
}

impl OpenRequest {
    pub fn new(ch: Rc<Channel>, txid: TxId) -> Self {
        Self { ch, txid }
    }

    pub fn error(self, error: ErrorCode) -> Result<(), SendError> {
        self.ch.reply(Reply::ErrorReply { error })
    }

    pub fn complete(self, new_ch: Channel) -> Result<(), SendError> {
        self.ch.reply(Reply::OpenReply { ch: new_ch })
    }
}

pub trait Application {
    fn init() -> Self;
    fn open(&mut self, req: OpenRequest);
    // fn read(&mut self, req: Request<ReadReply>);
    // fn write(&mut self, req: Request<WriteReply>);
    fn open_reply(&mut self, new_ch: Channel);
    fn read_reply(&mut self, buf: BufferMut, len: usize);
    fn write_reply(&mut self, buf: Buffer, len: usize);
    fn error_reply(&mut self, error: ErrorCode);
}

pub fn main<A: Application>(app: A) {
    let mut app = A::init();
    let mut sink = Sink::new().unwrap();
    let mut channels: HashMap<HandleId, Rc<Channel>> = HashMap::new();
    loop {
        match sink.pop().expect("failed to read an event from sink") {
            Event::Message { id, msginfo } => {}
        }
    }
}
