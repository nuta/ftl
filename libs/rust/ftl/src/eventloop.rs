use alloc::boxed::Box;
use alloc::rc::Rc;
use core::marker::PhantomData;

use ftl_types::channel::CallId;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::MessageInlineBody;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;
use log::warn;

use crate::channel::Channel;
use crate::channel::Reply;
use crate::handle::Handleable;
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
    ReadUri {
        ctx: &'a mut C,
        completer: ReadUriCompleter,
        offset: usize,
        len: usize,
    },
    WriteUri {
        ctx: &'a mut C,
        completer: WriteUriCompleter,
        offset: usize,
        len: usize,
    },
    OpenReply {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
        cookie: K,
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
    ReadUriReply {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
        cookie: K,
        len: usize,
    },
    WriteUriReply {
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

enum Object {
    Channel(Rc<Channel>),
    Interrupt(Rc<Interrupt>),
}

struct Entry<C> {
    object: Object,
    ctx: C,
}

pub trait SmartPointer {
    fn into_raw(self) -> usize;
    fn from_raw(raw: usize) -> Self;
}

impl<T> SmartPointer for Box<T> {
    fn into_raw(self) -> usize {
        Box::into_raw(self) as usize
    }

    fn from_raw(raw: usize) -> Self {
        unsafe { Box::from_raw(raw as *mut T) }
    }
}

impl<T> SmartPointer for Rc<T> {
    fn into_raw(self) -> usize {
        Rc::into_raw(self) as usize
    }

    fn from_raw(raw: usize) -> Self {
        unsafe { Rc::from_raw(raw as *mut T) }
    }
}

pub struct EventLoop<C, K: SmartPointer> {
    sink: Sink,
    entries: HashMap<HandleId, Entry<C>>,
    _pd: PhantomData<K>,
}

impl<C, K: SmartPointer> EventLoop<C, K> {
    pub fn new() -> Result<Self, Error> {
        let sink = Sink::new().map_err(Error::SinkCreate)?;
        Ok(Self {
            sink,
            entries: HashMap::new(),
            _pd: PhantomData,
        })
    }

    pub fn add_channel(&mut self, channel: impl Into<Rc<Channel>>, ctx: C) -> Result<(), Error> {
        let ch = channel.into();
        self.sink.add(ch.as_ref()).map_err(Error::SinkAdd)?;
        self.entries.insert(
            ch.handle().id(),
            Entry {
                object: Object::Channel(ch),
                ctx,
            },
        );
        Ok(())
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
        let result = self.sink.wait();
        match result {
            Ok(sink::Event::CallMessage {
                ch_id,
                info,
                call_id,
                handles,
                inline,
            }) => {
                let (ch, ctx) = match self.entries.get_mut(&ch_id) {
                    Some(Entry {
                        object: Object::Channel(ch),
                        ctx,
                    }) => (ch.clone(), ctx),
                    _ => panic!("unknown handle id from sink: {:?}", ch_id),
                };

                // FIXME: Guarantee the alignment of the inline body.
                let inline_body = unsafe { &*(inline.as_ptr() as *const MessageInlineBody) };
                match info {
                    MessageInfo::OPEN => {
                        Event::Open {
                            ctx,
                            completer: OpenCompleter { ch, call_id },
                        }
                    }
                    MessageInfo::READ => {
                        Event::Read {
                            ctx,
                            offset: unsafe { inline_body.read.offset },
                            len: unsafe { inline_body.read.len },
                            completer: ReadCompleter { ch, call_id },
                        }
                    }
                    MessageInfo::WRITE => {
                        Event::Write {
                            ctx,
                            offset: unsafe { inline_body.write.offset },
                            len: unsafe { inline_body.write.len },
                            completer: WriteCompleter { ch, call_id },
                        }
                    }
                    MessageInfo::READ_URI => {
                        Event::ReadUri {
                            ctx,
                            offset: unsafe { inline_body.read_uri.offset },
                            len: unsafe { inline_body.read_uri.len },
                            completer: ReadUriCompleter { ch, call_id },
                        }
                    }
                    MessageInfo::WRITE_URI => {
                        Event::WriteUri {
                            ctx,
                            offset: unsafe { inline_body.write_uri.offset },
                            len: unsafe { inline_body.write_uri.len },
                            completer: WriteUriCompleter { ch, call_id },
                        }
                    }
                    _ => Event::UnknownMessage { ctx, info },
                }
            }
            Ok(sink::Event::ReplyMessage {
                ch_id,
                info,
                cookie,
                handles,
                inline,
            }) => {
                let (ch, ctx) = match self.entries.get_mut(&ch_id) {
                    Some(Entry {
                        object: Object::Channel(ch),
                        ctx,
                    }) => (ch, ctx),
                    _ => panic!("unknown handle id from sink: {:?}", ch_id),
                };

                // FIXME: Guarantee the alignment of the inline body.
                let inline_body = unsafe { &*(inline.as_ptr() as *const MessageInlineBody) };
                let cookie = K::from_raw(cookie);
                match info {
                    MessageInfo::OPEN_REPLY => Event::OpenReply { ctx, ch, cookie },
                    MessageInfo::READ_REPLY => {
                        let len = unsafe { inline_body.read_reply.len };
                        Event::ReadReply {
                            ctx,
                            ch,
                            cookie,
                            len,
                        }
                    }
                    MessageInfo::WRITE_REPLY => {
                        let len = unsafe { inline_body.write_reply.len };
                        Event::WriteReply {
                            ctx,
                            ch,
                            cookie,
                            len,
                        }
                    }
                    MessageInfo::READ_URI_REPLY => {
                        let len = unsafe { inline_body.read_uri_reply.len };
                        Event::ReadUriReply {
                            ctx,
                            ch,
                            cookie,
                            len,
                        }
                    }
                    MessageInfo::WRITE_URI_REPLY => {
                        let len = unsafe { inline_body.write_uri_reply.len };
                        Event::WriteUriReply {
                            ctx,
                            ch,
                            cookie,
                            len,
                        }
                    }
                    MessageInfo::ERROR_REPLY => {
                        let error = unsafe { inline_body.error_reply.error };
                        Event::ErrorReply {
                            ctx,
                            ch,
                            cookie,
                            error,
                        }
                    }
                    _ => Event::UnknownMessage { ctx, info },
                }
            }
            Ok(sink::Event::PeerClosed { ch_id }) => {
                match self.entries.get_mut(&ch_id) {
                    Some(Entry {
                        object: Object::Channel(ch),
                        ctx,
                    }) => Event::PeerClosed { ctx, ch },
                    _ => panic!("unknown handle id from sink: {:?}", ch_id),
                }
            }
            Ok(sink::Event::Irq { handle_id, irq }) => {
                match self.entries.get_mut(&handle_id) {
                    Some(Entry {
                        object: Object::Interrupt(interrupt),
                        ctx,
                    }) => Event::Irq { ctx, interrupt },
                    _ => panic!("unknown handle id from sink: {:?}", handle_id),
                }
            }
            Ok(sink::Event::Timer { handle_id }) => {
                todo!()
            }
            Ok(sink::Event::SandboxedSyscall { thread_id, raw }) => {
                todo!()
            }
            Err(error) => Event::SinkError(error),
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
        if let Err(error) = self.ch.reply(self.call_id, Reply::OpenReply { ch }) {
            warn!("failed to complete open: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(error) = self.ch.reply(self.call_id, Reply::ErrorReply { error }) {
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
        if let Err(error) = self.ch.reply(self.call_id, Reply::ErrorReply { error }) {
            warn!("failed to error read: {:?}", error);
        }
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, len: usize) {
        if let Err(error) = self.ch.reply(self.call_id, Reply::ReadReply { len }) {
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
        if let Err(error) = self.ch.reply(self.call_id, Reply::WriteReply { len }) {
            warn!("failed to complete write: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(error) = self.ch.reply(self.call_id, Reply::ErrorReply { error }) {
            warn!("failed to error write: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct ReadUriCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl ReadUriCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn read_uri(&self, offset: usize, uri: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, uri)
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 1, offset, data)
    }

    pub fn complete(&self, len: usize) {
        if let Err(error) = self.ch.reply(self.call_id, Reply::ReadUriReply { len }) {
            warn!("failed to complete read uri: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(error) = self.ch.reply(self.call_id, Reply::ErrorReply { error }) {
            warn!("failed to error read uri: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct WriteUriCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl WriteUriCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn read_uri(&self, offset: usize, uri: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, uri)
    }

    pub fn read(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 1, offset, data)
    }

    pub fn complete(&self, len: usize) {
        if let Err(error) = self.ch.reply(self.call_id, Reply::WriteUriReply { len }) {
            warn!("failed to complete write uri: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(error) = self.ch.reply(self.call_id, Reply::ErrorReply { error }) {
            warn!("failed to error write uri: {:?}", error);
        }
    }
}
