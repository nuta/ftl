use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use ftl_types::channel::ErrorReplyInline;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::ReadInline;
use ftl_types::channel::ReadReplyInline;
use ftl_types::channel::TxId;
use ftl_types::channel::WriteInline;
use ftl_types::channel::WriteReplyInline;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Buffer;
use crate::channel::BufferMut;
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

pub enum Cookie {
    Buffer(Buffer),
    BufferMut(BufferMut),
}

fn cookie_to_buffer_mut(cookie: usize) -> BufferMut {
    todo!()
}

fn cookie_to_buffer(cookie: usize) -> Buffer {
    todo!()
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
                let cookie: Box<Cookie> = unsafe { Box::from_raw(cookie as *mut Cookie) };
                let (ch, session) = channels.get_mut(&id).unwrap();
                let mut ctx = Context { session };
                match msginfo {
                    MessageInfo::OPEN => {
                        app.open(&mut ctx, OpenRequest::new(ch.clone(), txid));
                    }
                    MessageInfo::READ => {
                        let inline = msg.inline::<ReadInline>();
                        app.read(&mut ctx, ReadRequest::new(ch.clone(), txid));
                    }
                    MessageInfo::WRITE => {
                        let inline = msg.inline::<WriteInline>();
                        app.write(&mut ctx, WriteRequest::new(ch.clone(), txid));
                    }
                    MessageInfo::OPEN_REPLY => {
                        let new_ch = Channel::from_handle(OwnedHandle::from_raw(msg.handles[0]));
                        app.open_reply(&mut ctx, new_ch);
                    }
                    MessageInfo::READ_REPLY => {
                        let inline = msg.inline::<ReadReplyInline>();
                        let Cookie::BufferMut(buf) = *cookie else {
                            unreachable!()
                        };
                        app.read_reply(&mut ctx, buf, inline.len);
                    }
                    MessageInfo::WRITE_REPLY => {
                        let inline = msg.inline::<WriteReplyInline>();
                        let Cookie::Buffer(buf) = *cookie else {
                            unreachable!()
                        };
                        app.write_reply(&mut ctx, buf, inline.len);
                    }
                    MessageInfo::ERROR_REPLY => {
                        let inline = msg.inline::<ErrorReplyInline>();
                        app.error_reply(&mut ctx, inline.error);
                    }
                    _ => {
                        println!("unknown message type: {:?}", msginfo);
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
