#![allow(unused)]
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;

use ftl_types::channel::Attr;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::RawMessage;
use ftl_types::channel::RequestId;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::EventType;
use hashbrown::HashMap;
use log::warn;

use crate::buffer::Buffer;
use crate::buffer::BufferMut;
use crate::buffer::BufferUninit;
use crate::channel::Channel;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::interrupt::Interrupt;
use crate::sink;
use crate::sink::SandboxedSyscallEvent;
use crate::sink::Sink;
use crate::thread::Thread;
use crate::time::Timer;

#[derive(Debug)]
pub enum Event<'a, C, K: 'static> {
    Request {
        ctx: &'a mut C,
        request: Request,
    },
    Reply {
        ctx: &'a mut C,
        reply: Reply<K>,
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
    SandboxedSyscall {
        ctx: &'a mut C,
        thread: &'a Rc<Thread>,
        regs: SandboxedSyscallEvent,
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
    Thread(Rc<Thread>),
}

struct Entry<C> {
    object: Object,
    ctx: C,
}

pub struct EventLoop<C, K: 'static> {
    sink: Sink,
    entries: HashMap<HandleId, Entry<C>>,
    _pd: PhantomData<K>,
}

impl<C, K: 'static> EventLoop<C, K> {
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

    pub fn add_thread(&mut self, thread: impl Into<Rc<Thread>>, ctx: C) -> Result<(), Error> {
        let thread = thread.into();
        self.sink.add(thread.as_ref()).map_err(Error::SinkAdd)?;
        self.entries.insert(
            thread.handle().id(),
            Entry {
                object: Object::Thread(thread),
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
                        Event::Request {
                            ctx,
                            request: Request::Open(OpenRequest {
                                ch: ch.clone(),
                                request_id: body.request_id,
                                path_len: body.body_len,
                            }),
                        }
                    }
                    MessageInfo::READ => {
                        Event::Request {
                            ctx,
                            request: Request::Read(ReadRequest {
                                ch: ch.clone(),
                                request_id: body.request_id,
                                offset: body.inline,
                                body_len: body.body_len,
                            }),
                        }
                    }
                    MessageInfo::WRITE => {
                        Event::Request {
                            ctx,
                            request: Request::Write(WriteRequest {
                                ch: ch.clone(),
                                request_id: body.request_id,
                                offset: body.inline,
                                body_len: body.body_len,
                            }),
                        }
                    }
                    MessageInfo::GETATTR => {
                        Event::Request {
                            ctx,
                            request: Request::GetAttr(GetAttrRequest {
                                ch: ch.clone(),
                                request_id: body.request_id,
                                attr: Attr::from_usize(body.inline),
                                body_len: body.body_len,
                            }),
                        }
                    }
                    MessageInfo::SETATTR => {
                        Event::Request {
                            ctx,
                            request: Request::SetAttr(SetAttrRequest {
                                ch: ch.clone(),
                                request_id: body.request_id,
                                attr: Attr::from_usize(body.inline),
                                body_len: body.body_len,
                            }),
                        }
                    }
                    MessageInfo::OPEN_REPLY => {
                        let (cookie, path) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::Buffer(path) = path else {
                            unreachable!()
                        };

                        let new_ch = Channel::from_handle(OwnedHandle::from_raw(body.handle));
                        Event::Reply {
                            ctx,
                            reply: Reply::Open {
                                client: Client::new(ch.clone()),
                                cookie,
                                path,
                                new_ch,
                            },
                        }
                    }
                    MessageInfo::READ_REPLY => {
                        let (cookie, buf) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::BufferUninit(buf) = buf else {
                            unreachable!()
                        };

                        let len = body.inline;
                        let buf = unsafe { buf.assume_init(len) };
                        Event::Reply {
                            ctx,
                            reply: Reply::Read {
                                client: Client::new(ch.clone()),
                                cookie,
                                buf,
                                len,
                            },
                        }
                    }
                    MessageInfo::WRITE_REPLY => {
                        let (cookie, buf) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::Buffer(buf) = buf else {
                            unreachable!()
                        };

                        Event::Reply {
                            ctx,
                            reply: Reply::Write {
                                client: Client::new(ch.clone()),
                                cookie,
                                buf,
                                len: body.inline,
                            },
                        }
                    }
                    MessageInfo::GETATTR_REPLY => {
                        let (cookie, buf) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::BufferUninit(buf) = buf else {
                            unreachable!()
                        };

                        let len = body.inline;
                        let buf = unsafe { buf.assume_init(len) };
                        Event::Reply {
                            ctx,
                            reply: Reply::GetAttr {
                                client: Client::new(ch.clone()),
                                cookie,
                                buf,
                                len,
                            },
                        }
                    }
                    MessageInfo::SETATTR_REPLY => {
                        let (cookie, buf) = CookieWrapper::from_raw(body.cookie);
                        let BufferWrapper::Buffer(buf) = buf else {
                            unreachable!()
                        };

                        Event::Reply {
                            ctx,
                            reply: Reply::SetAttr {
                                client: Client::new(ch.clone()),
                                cookie,
                                buf,
                                len: body.inline,
                            },
                        }
                    }
                    MessageInfo::ERROR_REPLY => {
                        let (cookie, _buf) = CookieWrapper::from_raw(body.cookie);
                        Event::Reply {
                            ctx,
                            reply: Reply::Error {
                                client: Client::new(ch.clone()),
                                cookie,
                                error: ErrorCode::from(body.inline),
                            },
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
                let Object::Thread(thread) = object else {
                    panic!("expected thread object, got {:?}", object);
                };

                let regs = unsafe { event.body.sandboxed_syscall };
                Event::SandboxedSyscall { ctx, thread, regs }
            }
            _ => panic!("unknown event type from sink: {:?}", event.header.ty),
        }
    }
}

#[derive(Debug)]
pub struct OpenRequest {
    ch: Rc<Channel>,
    request_id: RequestId,
    path_len: usize,
}

impl OpenRequest {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn path_len(&self) -> usize {
        self.path_len
    }

    pub fn path(&self, buf: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.read_body(self.request_id, 0, buf)
    }

    pub fn reply(self, ch: Channel) {
        let mut body = new_message_body();
        body.handle = ch.handle().id();
        match self
            .ch
            .reply(MessageInfo::OPEN_REPLY, &body, self.request_id)
        {
            Ok(()) => {
                mem::forget(ch);
            }
            Err(error) => {
                warn!("failed to reply open: {:?}", error);
            }
        }
    }

    pub fn reply_error(self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self
            .ch
            .reply(MessageInfo::ERROR_REPLY, &body, self.request_id)
        {
            warn!("failed to reply open error: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct ReadRequest {
    ch: Rc<Channel>,
    request_id: RequestId,
    offset: usize,
    body_len: usize,
}

impl ReadRequest {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn len(&self) -> usize {
        self.body_len
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, ErrorCode> {
        self.write_at(buf, 0)
    }

    pub fn write_at(&self, buf: &[u8], offset: usize) -> Result<usize, ErrorCode> {
        self.ch.write_body(self.request_id, offset, buf)
    }

    pub fn reply(self, len: usize) {
        let mut body = new_message_body();
        body.inline = len;
        if let Err(error) = self
            .ch
            .reply(MessageInfo::READ_REPLY, &body, self.request_id)
        {
            warn!("failed to reply read: {:?}", error);
        }
    }

    pub fn reply_error(self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self
            .ch
            .reply(MessageInfo::ERROR_REPLY, &body, self.request_id)
        {
            warn!("failed to reply read error: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct WriteRequest {
    ch: Rc<Channel>,
    request_id: RequestId,
    offset: usize,
    body_len: usize,
}

impl WriteRequest {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn len(&self) -> usize {
        self.body_len
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, ErrorCode> {
        self.read_at(buf, 0)
    }

    pub fn read_at(&self, buf: &mut [u8], offset: usize) -> Result<usize, ErrorCode> {
        self.ch.read_body(self.request_id, offset, buf)
    }

    pub fn reply(self, len: usize) {
        let mut body = new_message_body();
        body.inline = len;
        if let Err(error) = self
            .ch
            .reply(MessageInfo::WRITE_REPLY, &body, self.request_id)
        {
            warn!("failed to reply write: {:?}", error);
        }
    }

    pub fn reply_error(self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self
            .ch
            .reply(MessageInfo::ERROR_REPLY, &body, self.request_id)
        {
            warn!("failed to reply write error: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct GetAttrRequest {
    ch: Rc<Channel>,
    request_id: RequestId,
    attr: Attr,
    body_len: usize,
}

impl GetAttrRequest {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn attr(&self) -> Attr {
        self.attr
    }

    pub fn len(&self) -> usize {
        self.body_len
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, ErrorCode> {
        self.write_at(buf, 0)
    }

    pub fn write_at(&self, buf: &[u8], offset: usize) -> Result<usize, ErrorCode> {
        self.ch.write_body(self.request_id, offset, buf)
    }

    pub fn reply(self, len: usize) {
        let mut body = new_message_body();
        body.inline = len;
        if let Err(error) = self
            .ch
            .reply(MessageInfo::GETATTR_REPLY, &body, self.request_id)
        {
            warn!("failed to reply getattr: {:?}", error);
        }
    }

    pub fn reply_error(self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self
            .ch
            .reply(MessageInfo::ERROR_REPLY, &body, self.request_id)
        {
            warn!("failed to reply getattr error: {:?}", error);
        }
    }
}

#[derive(Debug)]
pub struct SetAttrRequest {
    ch: Rc<Channel>,
    request_id: RequestId,
    attr: Attr,
    body_len: usize,
}

impl SetAttrRequest {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn attr(&self) -> Attr {
        self.attr
    }

    pub fn len(&self) -> usize {
        self.body_len
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, ErrorCode> {
        self.read_at(buf, 0)
    }

    pub fn read_at(&self, buf: &mut [u8], offset: usize) -> Result<usize, ErrorCode> {
        self.ch.read_body(self.request_id, offset, buf)
    }

    pub fn reply(self, len: usize) {
        let mut body = new_message_body();
        body.inline = len;
        if let Err(error) = self
            .ch
            .reply(MessageInfo::SETATTR_REPLY, &body, self.request_id)
        {
            warn!("failed to reply setattr: {:?}", error);
        }
    }

    pub fn reply_error(self, error: ErrorCode) {
        let mut body = new_message_body();
        body.inline = error.as_usize();
        if let Err(error) = self
            .ch
            .reply(MessageInfo::ERROR_REPLY, &body, self.request_id)
        {
            warn!("failed to reply setattr error: {:?}", error);
        }
    }
}

enum BufferWrapper {
    Buffer(Buffer),
    BufferUninit(BufferUninit),
}

struct CookieWrapper<K: 'static>(Box<(K, BufferWrapper)>);

impl<K: 'static> CookieWrapper<K> {
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

fn new_message_body() -> RawMessage {
    unsafe { MaybeUninit::<RawMessage>::zeroed().assume_init() }
}

#[derive(Debug)]
pub struct Client<K: 'static> {
    ch: Rc<Channel>,
    _cookie: PhantomData<K>,
}

impl<K: 'static> Clone for Client<K> {
    fn clone(&self) -> Self {
        Self {
            ch: self.ch.clone(),
            _cookie: PhantomData,
        }
    }
}

impl<K: 'static> Client<K> {
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
        body: &RawMessage,
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
        body.body_addr = addr;
        body.body_len = len;

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::Buffer(path));
        self.call_with_cookie(MessageInfo::OPEN, &body, wrapper)
    }

    /// Sends a read request.
    pub fn read(
        &self,
        offset: usize,
        data: impl Into<BufferUninit>,
        cookie: K,
    ) -> Result<(), ErrorCode> {
        let mut body = new_message_body();

        let mut data = data.into();
        let (addr, len) = data.addr_and_len();
        body.body_addr = addr;
        body.body_len = len;
        body.inline = offset;

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::BufferUninit(data));
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
        body.body_addr = addr;
        body.body_len = len;
        body.inline = offset;

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::Buffer(data));
        self.call_with_cookie(MessageInfo::WRITE, &body, wrapper)
    }

    /// Sends a getattr request.
    pub fn getattr(
        &self,
        attr: Attr,
        data: impl Into<BufferUninit>,
        cookie: K,
    ) -> Result<(), ErrorCode> {
        let mut body = new_message_body();

        let mut data = data.into();
        let (addr, len) = data.addr_and_len();
        body.body_addr = addr;
        body.body_len = len;
        body.inline = attr.as_usize();

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::BufferUninit(data));
        self.call_with_cookie(MessageInfo::GETATTR, &body, wrapper)
    }

    /// Sends a setattr request.
    pub fn setattr(&self, attr: Attr, data: impl Into<Buffer>, cookie: K) -> Result<(), ErrorCode> {
        let mut body = new_message_body();

        let data = data.into();
        let (addr, len) = data.addr_and_len();
        body.body_addr = addr;
        body.body_len = len;
        body.inline = attr.as_usize();

        let wrapper = CookieWrapper::new(cookie, BufferWrapper::Buffer(data));
        self.call_with_cookie(MessageInfo::SETATTR, &body, wrapper)
    }
}

#[derive(Debug)]
pub enum Request {
    Open(OpenRequest),
    Read(ReadRequest),
    Write(WriteRequest),
    GetAttr(GetAttrRequest),
    SetAttr(SetAttrRequest),
}

#[derive(Debug)]
pub enum Reply<K: 'static> {
    Open {
        client: Client<K>,
        cookie: K,
        path: Buffer,
        new_ch: Channel,
    },
    Read {
        client: Client<K>,
        cookie: K,
        buf: BufferMut,
        len: usize,
    },
    Write {
        client: Client<K>,
        cookie: K,
        buf: Buffer,
        len: usize,
    },
    GetAttr {
        client: Client<K>,
        cookie: K,
        buf: BufferMut,
        len: usize,
    },
    SetAttr {
        client: Client<K>,
        cookie: K,
        buf: Buffer,
        len: usize,
    },
    Error {
        client: Client<K>,
        cookie: K,
        error: ErrorCode,
    },
}

fn reply_error(ch: &Rc<Channel>, request_id: RequestId, error: ErrorCode) {
    let mut body = new_message_body();
    body.inline = error.as_usize();
    if let Err(err) = ch.reply(MessageInfo::ERROR_REPLY, &body, request_id) {
        warn!("failed to reply error: {:?}", err);
    }
}

fn reply_value(ch: &Rc<Channel>, info: MessageInfo, request_id: RequestId, value: usize) {
    let mut body = new_message_body();
    body.inline = value;
    if let Err(err) = ch.reply(info, &body, request_id) {
        warn!("failed to reply {:?}: {:?}", info, err);
    }
}
