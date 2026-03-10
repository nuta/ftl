#![allow(unused)]
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;

use ftl_types::channel::Attr;
use ftl_types::channel::CallId;
use ftl_types::channel::MessageBody;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::MessageInlineBody;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::EventType;
use hashbrown::HashMap;
use log::warn;

use crate::channel::Buffer;
use crate::channel::BufferMut;
use crate::channel::Channel;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::interrupt::Interrupt;
use crate::sink;
use crate::sink::Sink;

#[derive(Debug)]
pub enum Event<'a, C, K> {
    Open {
        ctx: &'a mut C,
        completer: OpenCompleter,
    },
    Read {
        ctx: &'a mut C,
        completer: ReadCompleter,
        offset: usize,
        len: usize,
    },
    Write {
        ctx: &'a mut C,
        completer: WriteCompleter,
        offset: usize,
        len: usize,
    },
    Getattr {
        ctx: &'a mut C,
        completer: GetattrCompleter,
        attr: Attr,
        len: usize,
    },
    Setattr {
        ctx: &'a mut C,
        completer: SetattrCompleter,
        attr: Attr,
        len: usize,
    },
    OpenReply {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
        cookie: K,
        new_ch: Channel,
    },
    ReadReply {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
        cookie: K,
        len: usize,
    },
    WriteReply {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
        cookie: K,
        len: usize,
    },
    GetattrReply {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
        cookie: K,
        len: usize,
    },
    SetattrReply {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
        cookie: K,
        len: usize,
    },
    ErrorReply {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
        cookie: K,
        error: ErrorCode,
    },
    PeerClosed {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
    },
    UnknownMessage {
        ctx: &'a mut C,
        info: MessageInfo,
    },
    Irq {
        ctx: &'a mut C,
        interrupt: &'a Rc<Interrupt>,
    },
    SinkError(ErrorCode),
}

#[derive(Debug)]
pub enum Error {
    SinkCreate(ErrorCode),
    SinkRemove(ErrorCode),
    SinkAdd(ErrorCode),
}

#[derive(Debug)]
enum Object {
    Channel(Rc<Channel>),
    Interrupt(Rc<Interrupt>),
}

struct Entry<C> {
    object: Object,
    ctx: C,
}

pub struct EventLoop<C, K> {
    sink: Sink,
    entries: HashMap<HandleId, Entry<C>>,
    _pd: PhantomData<K>,
}

impl<C, K> EventLoop<C, K> {
    pub fn new() -> Result<Self, Error> {
        let sink = Sink::new().map_err(Error::SinkCreate)?;
        Ok(Self {
            sink,
            entries: HashMap::new(),
            _pd: PhantomData,
        })
    }

    pub fn add_channel(
        &mut self,
        channel: impl Into<Rc<Channel>>,
        ctx: C,
    ) -> Result<Client<K>, Error> {
        let ch: Rc<Channel> = channel.into();
        self.sink.add(ch.as_ref()).map_err(Error::SinkAdd)?;
        self.entries.insert(
            ch.handle().id(),
            Entry {
                object: Object::Channel(ch.clone()),
                ctx,
            },
        );

        Ok(Client::new(ch))
    }

    pub fn add_interrupt(
        &mut self,
        interrupt: impl Into<Rc<Interrupt>>,
        ctx: C,
    ) -> Result<(), Error> {
        let interrupt = interrupt.into();
        self.sink.add(interrupt.as_ref()).map_err(Error::SinkAdd)?;
        self.entries.insert(
            interrupt.handle().id(),
            Entry {
                object: Object::Interrupt(interrupt),
                ctx,
            },
        );
        Ok(())
    }

    pub fn remove(&mut self, id: HandleId) -> Result<(), Error> {
        self.sink.remove(id).map_err(Error::SinkRemove)?;
        self.entries.remove(&id);
        Ok(())
    }

    pub fn wait(&mut self) -> Event<'_, C, K> {
        let mut raw = MaybeUninit::<sink::Event>::uninit();
        let event = match self.sink.wait(&mut raw) {
            Ok(event) => event,
            Err(error) => return Event::SinkError(error),
        };

        let (object, ctx) = match self.entries.get_mut(&event.header.id) {
            Some(Entry { object, ctx }) => (object, ctx),
            _ => panic!("unknown handle id from sink: {:?}", event.header.id),
        };

        match event.header.ty {
            EventType::MESSAGE => {
                let Object::Channel(ch) = object else {
                    panic!("expected channel object, got {:?}", object);
                };

                // FIXME: Guarantee the alignment of the inline body.
                let event_body = unsafe { &event.body.message };
                let inline = &unsafe { event_body.body.inline };
                match event_body.info {
                    MessageInfo::OPEN => {
                        Event::Open {
                            ctx,
                            completer: OpenCompleter {
                                ch: ch.clone(),
                                call_id: event_body.call_id,
                            },
                        }
                    }
                    MessageInfo::READ => {
                        Event::Read {
                            ctx,
                            offset: unsafe { inline.read.offset },
                            len: unsafe { inline.read.len },
                            completer: ReadCompleter {
                                ch: ch.clone(),
                                call_id: event_body.call_id,
                            },
                        }
                    }
                    MessageInfo::WRITE => {
                        Event::Write {
                            ctx,
                            offset: unsafe { inline.write.offset },
                            len: unsafe { inline.write.len },
                            completer: WriteCompleter {
                                ch: ch.clone(),
                                call_id: event_body.call_id,
                            },
                        }
                    }
                    MessageInfo::GETATTR => {
                        Event::Getattr {
                            ctx,
                            attr: unsafe { inline.getattr.attr },
                            len: unsafe { inline.getattr.len },
                            completer: GetattrCompleter {
                                ch: ch.clone(),
                                call_id: event_body.call_id,
                            },
                        }
                    }
                    MessageInfo::SETATTR => {
                        Event::Setattr {
                            ctx,
                            attr: unsafe { inline.setattr.attr },
                            len: unsafe { inline.setattr.len },
                            completer: SetattrCompleter {
                                ch: ch.clone(),
                                call_id: event_body.call_id,
                            },
                        }
                    }
                    MessageInfo::OPEN_REPLY => {
                        let wrapper = CookieWrapper::from_raw(event_body.cookie);
                        let new_ch_id = event_body.body.handle;
                        let new_ch = Channel::from_handle(OwnedHandle::from_raw(new_ch_id));
                        Event::OpenReply {
                            ctx,
                            ch,
                            cookie: wrapper.into_inner(),
                            new_ch,
                        }
                    }
                    MessageInfo::READ_REPLY => {
                        let wrapper = CookieWrapper::from_raw(event_body.cookie);
                        Event::ReadReply {
                            ctx,
                            ch,
                            cookie: wrapper.into_inner(),
                            len: unsafe { inline.read_reply.len },
                        }
                    }
                    MessageInfo::WRITE_REPLY => {
                        let wrapper = CookieWrapper::from_raw(event_body.cookie);
                        Event::WriteReply {
                            ctx,
                            ch,
                            cookie: wrapper.into_inner(),
                            len: unsafe { inline.write_reply.len },
                        }
                    }
                    MessageInfo::GETATTR_REPLY => {
                        let wrapper = CookieWrapper::from_raw(event_body.cookie);
                        Event::GetattrReply {
                            ctx,
                            ch,
                            cookie: wrapper.into_inner(),
                            len: unsafe { inline.getattr_reply.len },
                        }
                    }
                    MessageInfo::SETATTR_REPLY => {
                        let wrapper = CookieWrapper::from_raw(event_body.cookie);
                        Event::SetattrReply {
                            ctx,
                            ch,
                            cookie: wrapper.into_inner(),
                            len: unsafe { inline.setattr_reply.len },
                        }
                    }
                    MessageInfo::ERROR_REPLY => {
                        let wrapper = CookieWrapper::from_raw(event_body.cookie);
                        Event::ErrorReply {
                            ctx,
                            ch,
                            cookie: wrapper.into_inner(),
                            error: unsafe { inline.error_reply.error },
                        }
                    }
                    _ => {
                        Event::UnknownMessage {
                            ctx,
                            info: event_body.info,
                        }
                    }
                }
            }
            EventType::PEER_CLOSED => {
                let Object::Channel(ch) = object else {
                    panic!("expected channel object, got {:?}", object);
                };

                Event::PeerClosed { ctx, ch }
            }
            EventType::IRQ => {
                let Object::Interrupt(interrupt) = object else {
                    panic!("expected interrupt object, got {:?}", object);
                };

                Event::Irq { ctx, interrupt }
            }
            EventType::TIMER => {
                todo!()
            }
            EventType::SANDBOXED_SYSCALL => {
                todo!()
            }
            _ => panic!("unknown event type from sink: {:?}", event.header.ty),
        }
    }
}

#[derive(Debug)]
pub struct OpenCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl OpenCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn complete(&self, ch: Channel) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.handle = ch.handle().id();
        mem::forget(ch);

        if let Err(error) = self.ch.reply(MessageInfo::OPEN_REPLY, body, self.call_id) {
            warn!("failed to complete open: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.error_reply.error = error;
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, body, self.call_id) {
            warn!("failed to error open: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct ReadCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl ReadCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.error_reply.error = error;
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, body, self.call_id) {
            warn!("failed to error read: {:?}", error);
        }
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, len: usize) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.read_reply.len = len;
        if let Err(error) = self.ch.reply(MessageInfo::READ_REPLY, body, self.call_id) {
            warn!("failed to complete read: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct WriteCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl WriteCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }
}

impl WriteCompleter {
    pub fn read(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, len: usize) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.write_reply.len = len;
        if let Err(error) = self.ch.reply(MessageInfo::WRITE_REPLY, body, self.call_id) {
            warn!("failed to complete write: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.error_reply.error = error;
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, body, self.call_id) {
            warn!("failed to error write: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct GetattrCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl GetattrCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, len: usize) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.getattr_reply.len = len;
        if let Err(error) = self
            .ch
            .reply(MessageInfo::GETATTR_REPLY, body, self.call_id)
        {
            warn!("failed to complete getattr: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.error_reply.error = error;
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, body, self.call_id) {
            warn!("failed to error getattr: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct SetattrCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl SetattrCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn read(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, len: usize) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.setattr_reply.len = len;
        if let Err(error) = self
            .ch
            .reply(MessageInfo::SETATTR_REPLY, body, self.call_id)
        {
            warn!("failed to complete setattr: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.error_reply.error = error;
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, body, self.call_id) {
            warn!("failed to error setattr: {:?}", error);
        }
    }
}

pub struct CookieWrapper<K>(Box<K>);

impl<K> CookieWrapper<K> {
    pub fn new(cookie: K) -> Self {
        Self(Box::new(cookie))
    }

    pub fn into_raw(self) -> usize {
        Box::into_raw(self.0) as usize
    }

    pub fn from_raw(raw: usize) -> Self {
        Self(unsafe { Box::from_raw(raw as *mut K) })
    }

    pub fn into_inner(self) -> K {
        *self.0
    }
}

#[derive(Debug)]
pub struct Client<K> {
    ch: Rc<Channel>,
    _cookie: PhantomData<K>,
}

impl<K> Client<K> {
    pub fn new(ch: Rc<Channel>) -> Self {
        Self {
            ch,
            _cookie: PhantomData,
        }
    }

    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    /// Sends an open request.
    pub fn open(&self, path: impl Into<Buffer>, cookie: K) -> Result<(), ErrorCode> {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        todo!("data handling");

        let wrapper = CookieWrapper::new(cookie);
        self.ch.call(MessageInfo::OPEN, body, wrapper.into_raw())
    }

    /// Sends a read request.
    pub fn read(
        &self,
        offset: usize,
        data: impl Into<BufferMut>,
        cookie: K,
    ) -> Result<(), ErrorCode> {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.read.offset = offset;
        todo!("data handling");

        let wrapper = CookieWrapper::new(cookie);
        self.ch.call(MessageInfo::READ, body, wrapper.into_raw())
    }

    /// Sends a write request.
    pub fn write(
        &self,
        offset: usize,
        data: impl Into<Buffer>,
        cookie: K,
    ) -> Result<(), ErrorCode> {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.write.offset = offset;
        todo!("data handling");

        let wrapper = CookieWrapper::new(cookie);
        self.ch.call(MessageInfo::WRITE, body, wrapper.into_raw())
    }

    /// Sends a getattr request.
    pub fn getattr(
        &self,
        attr: Attr,
        data: impl Into<BufferMut>,
        cookie: K,
    ) -> Result<(), ErrorCode> {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.getattr.attr = attr;
        todo!("data handling");

        let wrapper = CookieWrapper::new(cookie);
        self.ch.call(MessageInfo::GETATTR, body, wrapper.into_raw())
    }

    /// Sends a setattr request.
    pub fn setattr(&self, attr: Attr, data: impl Into<Buffer>, cookie: K) -> Result<(), ErrorCode> {
        let mut body = MaybeUninit::<MessageBody>::uninit();
        let body = unsafe { body.assume_init_mut() };
        body.inline.setattr.attr = attr;
        todo!("data handling");

        let wrapper = CookieWrapper::new(cookie);
        self.ch.call(MessageInfo::SETATTR, body, wrapper.into_raw())
    }
}
