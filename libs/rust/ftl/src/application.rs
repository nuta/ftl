use alloc::rc::Rc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use ftl_types::channel::MSGTYPE_OPEN;
use ftl_types::channel::MSGTYPE_OPEN_REPLY;
use ftl_types::channel::MSGTYPE_READ;
use ftl_types::channel::MSGTYPE_READ_REPLY;
use ftl_types::channel::MSGTYPE_WRITE;
use ftl_types::channel::MSGTYPE_WRITE_REPLY;
use ftl_types::channel::ReadInline;
use ftl_types::channel::ReadReplyInline;
use ftl_types::channel::TxId;
use ftl_types::channel::WriteInline;
use ftl_types::channel::WriteReplyInline;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::buffer::Buffer;
use crate::buffer::BufferCookie;
use crate::buffer::BufferMut;
use crate::channel::Channel;
use crate::channel::Reply;
use crate::channel::SendError;
use crate::handle::OwnedHandle;
use crate::sink::Event;
use crate::sink::Sink;

pub struct Context<'a, A: Application> {
    session: &'a mut A::Session,
}

pub trait Application: Sized {
    type Session;

    fn init() -> Self;
    fn open(&mut self, ctx: &mut Context<Self>, req: OpenRequest);
    fn read(&mut self, ctx: &mut Context<Self>, req: ReadRequest);
    fn write(&mut self, ctx: &mut Context<Self>, req: WriteRequest);
    fn open_reply(&mut self, ctx: &mut Context<Self>, new_ch: Channel);
    fn read_reply(&mut self, ctx: &mut Context<Self>, buf: BufferMut, len: usize);
    fn write_reply(&mut self, ctx: &mut Context<Self>, buf: Buffer, len: usize);
    fn error_reply(&mut self, ctx: &mut Context<Self>, error: ErrorCode);
}

pub fn main<A: Application>(app: A) {
    let mut app = A::init();
    let mut sink = Sink::new().unwrap();
    let mut channels: HashMap<HandleId, (Rc<Channel>, A::Session)> = HashMap::new();
    loop {
        match sink.pop().expect("failed to read an event from sink") {
            Event::Message {
                id,
                msginfo,
                txid,
                cookie,
                msg,
            } => {
                let (ch, session) = channels.get_mut(&id).unwrap();
                let ctx = Context { session };
                match msginfo.ty() {
                    MSGTYPE_OPEN => {
                        app.open(&mut ctx, OpenRequest::new(ch.clone(), txid));
                    }
                    MSGTYPE_READ => {
                        let inline = msg.inline::<ReadInline>();
                        app.read(&mut ctx, ReadRequest::new(ch.clone(), txid));
                    }
                    MSGTYPE_WRITE => {
                        let inline = msg.inline::<WriteInline>();
                        app.write(&mut ctx, WriteRequest::new(ch.clone(), txid));
                    }
                    MSGTYPE_OPEN_REPLY => {
                        let new_ch = Channel::from_handle(OwnedHandle::from_raw(msg.handles[0]));
                        app.open_reply(&mut ctx, new_ch);
                    }
                    MSGTYPE_READ_REPLY => {
                        let inline = msg.inline::<ReadReplyInline>();
                        let cookie = BufferCookie::from_raw(cookie);
                        app.read_reply(&mut ctx, cookie.buf, inline.len);
                    }
                    MSGTYPE_WRITE_REPLY => {
                        let inline = msg.inline::<WriteReplyInline>();
                        let cookie = BufferCookie::from_raw(cookie);
                        app.write_reply(&mut ctx, cookie.buf, inline.len);
                    }
                }
            }
        }
    }
}

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

pub struct ReadRequest {
    ch: Rc<Channel>,
    txid: TxId,
}

impl ReadRequest {
    pub fn new(ch: Rc<Channel>, txid: TxId) -> Self {
        Self { ch, txid }
    }

    pub fn read_data(&self, data: &mut [u8], offset: usize) -> Result<(), ErrorCode> {
        todo!()
    }

    pub fn error(self, error: ErrorCode) -> Result<(), SendError> {
        self.ch.reply(Reply::ErrorReply { error })
    }

    pub fn complete(self, len: usize) -> Result<(), SendError> {
        self.ch.reply(Reply::ReadReply { len })
    }
}

pub struct WriteRequest {
    ch: Rc<Channel>,
    txid: TxId,
}

impl WriteRequest {
    pub fn new(ch: Rc<Channel>, txid: TxId) -> Self {
        Self { ch, txid }
    }

    pub fn write_data(&self, data: &[u8], offset: usize) -> Result<(), ErrorCode> {
        todo!()
    }

    pub fn error(self, error: ErrorCode) -> Result<(), SendError> {
        self.ch.reply(Reply::ErrorReply { error })
    }

    pub fn complete(self, len: usize) -> Result<(), SendError> {
        self.ch.reply(Reply::WriteReply { len })
    }
}
