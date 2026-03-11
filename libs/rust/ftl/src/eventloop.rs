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
use crate::time::Timer;

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
    GetAttr {
        ctx: &'a mut C,
        completer: GetAttrCompleter,
        attr: Attr,
        len: usize,
    },
    SetAttr {
        ctx: &'a mut C,
        completer: SetAttrCompleter,
        attr: Attr,
        len: usize,
    },
    OpenReply {
        ctx: &'a mut C,
        ch: &'a Rc<Channel>,
        cookie: K,
        uri: Buffer,
        new_ch: Channel,
    },
    ReadReply {
        ctx: &'a mut C,
        client: Client<K>,
        cookie: K,
        buf: BufferMut,
        len: usize,
    },
    WriteReply {
        ctx: &'a mut C,
        client: Client<K>,
        cookie: K,
        buf: Buffer,
        len: usize,
    },
    GetAttrReply {
        ctx: &'a mut C,
        client: Client<K>,
        cookie: K,
        buf: BufferMut,
        len: usize,
    },
    SetAttrReply {
        ctx: &'a mut C,
        client: Client<K>,
        cookie: K,
        buf: Buffer,
        len: usize,
    },
    ErrorReply {
        ctx: &'a mut C,
        client: Client<K>,
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
    Timer {
        ctx: &'a mut C,
        timer: &'a Rc<Timer>,
    },
    SinkError(ErrorCode),
}

#[derive(Debug)]
pub enum Error {
    SinkCreate(ErrorCode),
    SinkRemove(ErrorCode),
    SinkAdd(ErrorCode),
}

impl From<Error> for ErrorCode {
    fn from(error: Error) -> Self {
        match error {
            Error::SinkCreate(error) | Error::SinkRemove(error) | Error::SinkAdd(error) => error,
        }
    }
}

#[derive(Debug)]
enum Object {
    Channel(Rc<Channel>),
    Interrupt(Rc<Interrupt>),
    Timer(Rc<Timer>),
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
        // FIXME: Close or return the channel if the syscall fails.
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

    pub fn add_timer(&mut self, timer: impl Into<Rc<Timer>>, ctx: C) -> Result<(), Error> {
        let timer = timer.into();
        self.sink.add(timer.as_ref()).map_err(Error::SinkAdd)?;
        self.entries.insert(
            timer.handle().id(),
            Entry {
                object: Object::Timer(timer),
                ctx,
            },
        );
        Ok(())
    }

    pub fn remove(&mut self, id: HandleId) {
        if let Err(err) = self.sink.remove(id) {
            // Not much things we can do here.
            warn!("failed to remove handle {:?}: {:?}", id, err);
        }

        self.entries.remove(&id);
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
                let body = unsafe { &event.body.message };
                match body.info {
                    MessageInfo::OPEN => {
                        Event::Open {
                            ctx,
                            completer: OpenCompleter {
                                ch: ch.clone(),
                                call_id: body.call_id,
                            },
                        }
                    }
                    MessageInfo::READ => {
                        Event::Read {
                            ctx,
                            offset: body.inline,
                            len: body.ool_len,
                            completer: ReadCompleter {
                                ch: ch.clone(),
                                call_id: body.call_id,
                            },
                        }
                    }
                    MessageInfo::WRITE => {
                        Event::Write {
                            ctx,
                            offset: body.inline,
                            len: body.ool_len,
                            completer: WriteCompleter {
                                ch: ch.clone(),
                                call_id: body.call_id,
                            },
                        }
                    }
                    MessageInfo::GETATTR => {
                        Event::GetAttr {
                            ctx,
                            attr: Attr::from_usize(body.inline),
                            len: body.ool_len,
                            completer: GetAttrCompleter {
                                ch: ch.clone(),
                                call_id: body.call_id,
                            },
                        }
                    }
                    MessageInfo::SETATTR => {
                        Event::SetAttr {
                            ctx,
                            attr: Attr::from_usize(body.inline),
                            len: body.ool_len,
                            completer: SetAttrCompleter {
                                ch: ch.clone(),
                                call_id: body.call_id,
                            },
                        }
                    }
                    MessageInfo::OPEN_REPLY => {
                        let (cookie, uri) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::Buffer(uri) = uri else {
                            unreachable!()
                        };

                        let new_ch = Channel::from_handle(OwnedHandle::from_raw(body.handle));
                        Event::OpenReply {
                            ctx,
                            ch,
                            cookie,
                            uri,
                            new_ch,
                        }
                    }
                    MessageInfo::READ_REPLY => {
                        let (cookie, buf) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::BufferMut(buf) = buf else {
                            unreachable!()
                        };

                        Event::ReadReply {
                            ctx,
                            client: Client::new(ch.clone()),
                            cookie,
                            buf,
                            len: body.inline,
                        }
                    }
                    MessageInfo::WRITE_REPLY => {
                        let (cookie, buf) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::Buffer(buf) = buf else {
                            unreachable!()
                        };

                        Event::WriteReply {
                            ctx,
                            client: Client::new(ch.clone()),
                            cookie,
                            buf,
                            len: body.inline,
                        }
                    }
                    MessageInfo::GETATTR_REPLY => {
                        let (cookie, buf) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::BufferMut(buf) = buf else {
                            unreachable!()
                        };

                        Event::GetAttrReply {
                            ctx,
                            client: Client::new(ch.clone()),
                            cookie,
                            buf,
                            len: body.inline,
                        }
                    }
                    MessageInfo::SETATTR_REPLY => {
                        let (cookie, buf) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::Buffer(buf) = buf else {
                            unreachable!()
                        };

                        Event::SetAttrReply {
                            ctx,
                            client: Client::new(ch.clone()),
                            cookie,
                            buf,
                            len: body.inline,
                        }
                    }
                    MessageInfo::ERROR_REPLY => {
                        let (cookie, _buf) = CookieWrapper::from_raw(body.cookie);
                        Event::ErrorReply {
                            ctx,
                            client: Client::new(ch.clone()),
                            cookie,
                            error: ErrorCode::from(body.inline),
                        }
                    }
                    _ => {
                        Event::UnknownMessage {
                            ctx,
                            info: body.info,
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
                let Object::Timer(timer) = object else {
                    panic!("expected timer object, got {:?}", object);
                };

                Event::Timer { ctx, timer }
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

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn read_path(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, ch: Channel) {
        let mut body = new_message_body();
        body.handle = ch.handle().id();
        mem::forget(ch);

        if let Err(error) = self.ch.reply(MessageInfo::OPEN_REPLY, &body, self.call_id) {
            warn!("failed to complete open: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, &body, self.call_id) {
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

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, &body, self.call_id) {
            warn!("failed to error read: {:?}", error);
        }
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, len: usize) {
        let mut body = new_message_body();
        body.inline = len;
        if let Err(error) = self.ch.reply(MessageInfo::READ_REPLY, &body, self.call_id) {
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

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }
}

impl WriteCompleter {
    pub fn read(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, len: usize) {
        let mut body = new_message_body();
        body.inline = len;
        if let Err(error) = self.ch.reply(MessageInfo::WRITE_REPLY, &body, self.call_id) {
            warn!("failed to complete write: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, &body, self.call_id) {
            warn!("failed to error write: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct GetAttrCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl GetAttrCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, len: usize) {
        let mut body = new_message_body();
        body.inline = len;
        if let Err(error) = self
            .ch
            .reply(MessageInfo::GETATTR_REPLY, &body, self.call_id)
        {
            warn!("failed to complete getattr: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, &body, self.call_id) {
            warn!("failed to error getattr: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct SetAttrCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl SetAttrCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn read(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }

    pub fn complete(&self, len: usize) {
        let mut body = new_message_body();
        body.inline = len;
        if let Err(error) = self
            .ch
            .reply(MessageInfo::SETATTR_REPLY, &body, self.call_id)
        {
            warn!("failed to complete setattr: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self.ch.reply(MessageInfo::ERROR_REPLY, &body, self.call_id) {
            warn!("failed to error setattr: {:?}", error);
        }
    }
}

enum BufferWrapper {
    Buffer(Buffer),
    BufferMut(BufferMut),
}

struct CookieWrapper<K>(Box<(K, BufferWrapper)>);

impl<K> CookieWrapper<K> {
    fn new(cookie: K, buf: BufferWrapper) -> Self {
        Self(Box::new((cookie, buf)))
    }

    fn into_raw(self) -> usize {
        Box::into_raw(self.0) as usize
    }

    fn from_raw(raw: usize) -> (K, BufferWrapper) {
        let wrapper = unsafe { Box::from_raw(raw as *mut (K, BufferWrapper)) };
        *wrapper
    }
}

fn new_message_body() -> MessageBody {
    unsafe { MaybeUninit::<MessageBody>::zeroed().assume_init() }
}

#[derive(Debug)]
pub struct Client<K> {
    ch: Rc<Channel>,
    _cookie: PhantomData<K>,
}

impl<K> Clone for Client<K> {
    fn clone(&self) -> Self {
        Self {
            ch: self.ch.clone(),
            _cookie: PhantomData,
        }
    }
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

    fn call_with_cookie(
        &self,
        info: MessageInfo,
        body: &MessageBody,
        wrapper: CookieWrapper<K>,
    ) -> Result<(), ErrorCode> {
        let raw = wrapper.into_raw();
        match self.ch.call(info, body, raw) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = CookieWrapper::<K>::from_raw(raw);
                Err(error)
            }
        }
    }

    /// Sends an open request.
    pub fn open(&self, path: impl Into<Buffer>, cookie: K) -> Result<(), ErrorCode> {
        let mut body = new_message_body();

        let path = path.into();
        let (addr, len) = path.addr_and_len();
        body.ool_addr = addr;
        body.ool_len = len;

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::Buffer(path));
        self.call_with_cookie(MessageInfo::OPEN, &body, wrapper)
    }

    /// Sends a read request.
    pub fn read(
        &self,
        offset: usize,
        data: impl Into<BufferMut>,
        cookie: K,
    ) -> Result<(), ErrorCode> {
        let mut body = new_message_body();

        let data = data.into();
        let (addr, len) = data.addr_and_len();
        body.ool_addr = addr;
        body.ool_len = len;
        body.inline = offset;

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::BufferMut(data));
        self.call_with_cookie(MessageInfo::READ, &body, wrapper)
    }

    /// Sends a write request.
    pub fn write(
        &self,
        offset: usize,
        data: impl Into<Buffer>,
        cookie: K,
    ) -> Result<(), ErrorCode> {
        let mut body = new_message_body();

        let data = data.into();
        let (addr, len) = data.addr_and_len();
        body.ool_addr = addr;
        body.ool_len = len;
        body.inline = offset;

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::Buffer(data));
        self.call_with_cookie(MessageInfo::WRITE, &body, wrapper)
    }

    /// Sends a getattr request.
    pub fn getattr(
        &self,
        attr: Attr,
        data: impl Into<BufferMut>,
        cookie: K,
    ) -> Result<(), ErrorCode> {
        let mut body = new_message_body();

        let data = data.into();
        let (addr, len) = data.addr_and_len();
        body.ool_addr = addr;
        body.ool_len = len;
        body.inline = attr.as_usize();

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::BufferMut(data));
        self.call_with_cookie(MessageInfo::GETATTR, &body, wrapper)
    }

    /// Sends a setattr request.
    pub fn setattr(&self, attr: Attr, data: impl Into<Buffer>, cookie: K) -> Result<(), ErrorCode> {
        let mut body = new_message_body();

        let data = data.into();
        let (addr, len) = data.addr_and_len();
        body.ool_addr = addr;
        body.ool_len = len;
        body.inline = attr.as_usize();

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::Buffer(data));
        self.call_with_cookie(MessageInfo::SETATTR, &body, wrapper)
    }
}
