use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::cmp::min;
use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::panic::Location;
use core::ptr::null;
use core::ptr::null_mut;

pub use ftl_types::channel::Attr;
pub use ftl_types::channel::MessageId;
pub use ftl_types::channel::MessageInfo;
pub use ftl_types::channel::MessageKind;
pub use ftl_types::channel::OpenOptions;
use ftl_types::channel::Peek;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_DISCARD;
use ftl_types::syscall::SYS_CHANNEL_RECV;
use ftl_types::syscall::SYS_CHANNEL_SEND;
use log::debug;
use log::warn;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall1;
use crate::syscall::syscall2;
use crate::syscall::syscall3;
use crate::syscall::syscall5;

pub enum Message<'a> {
    Open {
        mid: MessageId,
        path: &'a [u8],
        options: OpenOptions,
    },
    Read {
        mid: MessageId,
        offset: usize,
        len: usize,
    },
    Write {
        mid: MessageId,
        offset: usize,
        buf: &'a [u8],
    },
    Getattr {
        mid: MessageId,
        attr: Attr,
    },
    Setattr {
        mid: MessageId,
        attr: Attr,
        buf: &'a [u8],
    },
    ErrorReply {
        mid: MessageId,
        error: ErrorCode,
    },
    OpenReply {
        mid: MessageId,
        handle: OwnedHandle,
    },
    ReadReply {
        mid: MessageId,
        buf: &'a [u8],
    },
    WriteReply {
        mid: MessageId,
        len: usize,
    },
    GetattrReply {
        mid: MessageId,
        buf: &'a [u8],
    },
    SetattrReply {
        mid: MessageId,
        len: usize,
    },
}

impl<C: ChannelRef> Incoming<C> {
    pub fn parse(ch: C, peek: Peek) -> Incoming<C> {
        match peek.info.kind() {
            MessageKind::OPEN => {
                let inner = RequestInner::new(ch, peek.info);
                let options = OpenOptions::from_usize(peek.arg1);
                Incoming::Open(OpenRequest::new(inner, options))
            }
            MessageKind::READ => {
                let inner = RequestInner::new(ch, peek.info);
                let offset = peek.arg1;
                let len = peek.arg2;
                Incoming::Read(ReadRequest::new(inner, offset, len))
            }
            MessageKind::WRITE => {
                let inner = RequestInner::new(ch, peek.info);
                let offset = peek.arg1;
                Incoming::Write(WriteRequest::new(inner, offset))
            }
            MessageKind::GETATTR => {
                todo!()
            }
            MessageKind::SETATTR => {
                todo!()
            }
            MessageKind::ERROR_REPLY => {
                let error = todo!();
                Incoming::ErrorReply(ErrorReply::new(ch, peek.info, error))
            }
            MessageKind::OPEN_REPLY => Incoming::OpenReply(OpenReply::new(ch, peek.info)),
            MessageKind::READ_REPLY => Incoming::ReadReply(ReadReply::new(ch, peek.info)),
            MessageKind::WRITE_REPLY => {
                let written_len = peek.arg1;
                Incoming::WriteReply(WriteReply::new(ch, peek.info, written_len))
            }
            MessageKind::GETATTR_REPLY => {
                todo!()
            }
            MessageKind::SETATTR_REPLY => {
                todo!()
            }
            _ => {
                todo!()
            }
        }
    }
}

pub struct Completer<'a, T: ?Sized> {
    ch: &'a Channel,
    reply_kind: MessageKind,
    mid: MessageId,
    _pd: PhantomData<T>,
}

impl<'a, T: ?Sized> Completer<'a, T> {
    fn new(ch: &'a Channel, reply_kind: MessageKind, mid: MessageId) -> Self {
        Self {
            ch,
            reply_kind,
            mid,
            _pd: PhantomData,
        }
    }

    pub fn reply_error(self, error: ErrorCode) -> Result<(), ErrorCode> {
        self.ch
            .send_args(self.reply_kind, self.mid, error.as_usize(), 0)?;
        Ok(())
    }
}

impl<'a> Completer<'a, usize> {
    pub fn reply(self, value: usize) -> Result<(), ErrorCode> {
        self.ch.send_args(self.reply_kind, self.mid, value, 0)?;
        Ok(())
    }
}

impl<'a> Completer<'a, [u8]> {
    pub fn reply(self, buf: &[u8]) -> Result<(), ErrorCode> {
        self.ch.send_body(self.reply_kind, self.mid, buf, 0)?;
        Ok(())
    }
}

impl<'a> Completer<'a, OwnedHandle> {
    pub fn reply(self, handle: OwnedHandle) -> Result<(), ErrorCode> {
        self.ch.send_handle(self.reply_kind, self.mid, handle)?;
        Ok(())
    }
}

pub struct OwnedCompleter<T: ?Sized> {
    ch: Rc<Channel>,
    reply_kind: MessageKind,
    mid: MessageId,
    _pd: PhantomData<T>,
}

impl<T: ?Sized> OwnedCompleter<T> {
    fn new(ch: Rc<Channel>, reply_kind: MessageKind, mid: MessageId) -> Self {
        Self {
            ch,
            reply_kind,
            mid,
            _pd: PhantomData,
        }
    }

    pub fn reply_error(self, error: ErrorCode) -> Result<(), ErrorCode> {
        self.ch
            .send_args(self.reply_kind, self.mid, error.as_usize(), 0)?;
        Ok(())
    }
}

impl OwnedCompleter<usize> {
    pub fn reply(self, value: usize) -> Result<(), ErrorCode> {
        self.ch.send_args(self.reply_kind, self.mid, value, 0)?;
        Ok(())
    }
}

impl OwnedCompleter<[u8]> {
    pub fn reply(self, buf: &[u8]) -> Result<(), ErrorCode> {
        self.ch.send_body(self.reply_kind, self.mid, buf, 0)?;
        Ok(())
    }
}

impl OwnedCompleter<OwnedHandle> {
    pub fn reply(self, handle: OwnedHandle) -> Result<(), ErrorCode> {
        self.ch.send_handle(self.reply_kind, self.mid, handle)?;
        Ok(())
    }
}

// ---------------------------------------------------------

pub trait ChannelRef {
    fn as_ref(&self) -> &Channel;
}

impl<'a> ChannelRef for &'a Channel {
    fn as_ref(&self) -> &Channel {
        self
    }
}

impl ChannelRef for Rc<Channel> {
    fn as_ref(&self) -> &Channel {
        &self
    }
}

impl<'a> ChannelRef for &'a Rc<Channel> {
    fn as_ref(&self) -> &Channel {
        &self
    }
}

impl<'a> ChannelRef for &'a mut Rc<Channel> {
    fn as_ref(&self) -> &Channel {
        &self
    }
}

struct RequestInner<C: ChannelRef> {
    ch: C,
    info: MessageInfo,
}

impl<C: ChannelRef> RequestInner<C> {
    fn new(ch: C, info: MessageInfo) -> Self {
        Self { ch, info }
    }

    fn mid(&self) -> MessageId {
        self.info.mid()
    }

    fn channel(&self) -> &C {
        &self.ch
    }

    fn recv_args(&self) -> Result<(), ErrorCode> {
        self.ch.as_ref().recv_args(self.info)?;
        Ok(())
    }

    fn recv_body<'a>(&self, body: &'a mut [u8]) -> Result<&'a [u8], ErrorCode> {
        self.ch.as_ref().recv_body(self.info, body)?;
        let read_len = min(body.len(), self.info.body_len());
        Ok(&body[..read_len])
    }

    fn discard_and_reply(self, m: Message) {
        if let Err(error) = self.ch.as_ref().discard(self.info) {
            debug!("failed to discard before reply: {:?}", error);
        }

        if let Err(error) = self.ch.as_ref().send(m) {
            debug!("failed to reply: {:?}", error);
        }
    }
}

struct CompleterInner<C: ChannelRef> {
    ch: C,
    mid: MessageId,
    completed: bool,
}

impl<C: ChannelRef> CompleterInner<C> {
    fn new(ch: C, mid: MessageId) -> Self {
        Self {
            ch,
            mid,
            completed: false,
        }
    }

    pub fn mid(&self) -> MessageId {
        self.mid
    }

    fn reply(mut self, m: Message) {
        self.completed = true;
        if let Err(error) = self.ch.as_ref().send(m) {
            debug!("failed to reply: {:?}", error);
        }
    }
}

impl<C: ChannelRef> Drop for CompleterInner<C> {
    #[track_caller]
    fn drop(&mut self) {
        if self.completed {
            return;
        }

        let caller = Location::caller();
        debug!(
            "completer dropped without reply at {}:{}: {:?}",
            caller.file(),
            caller.line(),
            self.mid
        );

        let m = Message::ErrorReply {
            mid: self.mid,
            error: ErrorCode::NotHandled,
        };
        if let Err(error) = self.ch.as_ref().send(m) {
            debug!("failed to reply: {:?}", error);
        }
    }
}

struct ReplyInner<C: ChannelRef> {
    ch: C,
    info: MessageInfo,
    received: bool,
}

impl<C: ChannelRef> ReplyInner<C> {
    fn new(ch: C, info: MessageInfo) -> Self {
        Self {
            ch,
            info,
            received: false,
        }
    }

    fn mid(&self) -> MessageId {
        self.info.mid()
    }

    fn recv_args(self) -> Result<(), ErrorCode> {
        self.ch.as_ref().recv_args(self.info)?;
        Ok(())
    }

    fn recv_handle(self) -> Result<OwnedHandle, ErrorCode> {
        self.ch.as_ref().recv_handle(self.info)
    }

    fn recv_body<'a>(&self, body: &'a mut [u8]) -> Result<&'a [u8], ErrorCode> {
        self.ch.as_ref().recv_body(self.info, body)?;
        let read_len = min(body.len(), self.info.body_len());
        Ok(&body[..read_len])
    }
}

impl<C: ChannelRef> Drop for ReplyInner<C> {
    fn drop(&mut self) {
        if self.received {
            return;
        }

        if let Err(error) = self.ch.as_ref().discard(self.info) {
            debug!("failed to discard before reply: {:?}", error);
        }
    }
}

pub struct OpenRequest<C: ChannelRef> {
    options: OpenOptions,
    inner: RequestInner<C>,
}

impl<C: ChannelRef> OpenRequest<C> {
    fn new(inner: RequestInner<C>, options: OpenOptions) -> Self {
        Self { inner, options }
    }

    pub fn options(&self) -> OpenOptions {
        self.options
    }

    pub fn path_len(&self) -> usize {
        self.inner.info.body_len()
    }

    pub fn recv<'a>(self, path: &'a mut [u8]) -> Result<(&'a [u8], OpenCompleter<C>), ErrorCode> {
        let body = self.inner.recv_body(path)?;
        let completer = OpenCompleter::new(self.inner);
        Ok((body, completer))
    }

    pub fn reply(self, handle: OwnedHandle) {
        let m = Message::OpenReply {
            mid: self.inner.mid(),
            handle,
        };
        self.inner.discard_and_reply(m);
    }

    pub fn reply_error(self, error: ErrorCode) {
        let m = Message::ErrorReply {
            mid: self.inner.mid(),
            error,
        };
        self.inner.discard_and_reply(m);
    }
}

pub struct OpenCompleter<C: ChannelRef>(CompleterInner<C>);

impl<C: ChannelRef> OpenCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        let mid = request.mid();
        Self(CompleterInner::new(request.ch, mid))
    }

    pub fn reply(mut self, handle: OwnedHandle) {
        let m = Message::OpenReply {
            mid: self.0.mid(),
            handle,
        };
        self.0.reply(m);
    }

    pub fn reply_error(mut self, error: ErrorCode) {
        let m = Message::ErrorReply {
            mid: self.0.mid(),
            error,
        };
        self.0.reply(m);
    }
}

pub struct ReadRequest<C: ChannelRef> {
    offset: usize,
    len: usize,
    inner: RequestInner<C>,
}

impl<C: ChannelRef> ReadRequest<C> {
    fn new(inner: RequestInner<C>, offset: usize, len: usize) -> Self {
        Self { inner, offset, len }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn recv<'a>(self) -> Result<ReadCompleter<C>, ErrorCode> {
        self.inner.recv_args()?;
        let completer = ReadCompleter::new(self.inner);
        Ok(completer)
    }

    pub fn reply(self, buf: &[u8]) {
        let mid = self.inner.mid();
        self.inner
            .discard_and_reply(Message::ReadReply { mid, buf });
    }
}

pub struct ReadCompleter<C: ChannelRef>(CompleterInner<C>);

impl<C: ChannelRef> ReadCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        let mid = request.mid();
        Self(CompleterInner::new(request.ch, mid))
    }

    pub fn reply(self, buf: &[u8]) {
        let mid = self.0.mid();
        self.0.reply(Message::ReadReply { mid, buf });
    }

    pub fn reply_error(self, error: ErrorCode) {
        let mid = self.0.mid();
        self.0.reply(Message::ErrorReply { mid, error });
    }
}

pub struct WriteRequest<C: ChannelRef> {
    offset: usize,
    inner: RequestInner<C>,
}

impl<C: ChannelRef> WriteRequest<C> {
    fn new(inner: RequestInner<C>, offset: usize) -> Self {
        Self { inner, offset }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn len(&self) -> usize {
        self.inner.info.body_len()
    }

    pub fn recv<'a>(self, buf: &'a mut [u8]) -> Result<(&'a [u8], WriteCompleter<C>), ErrorCode> {
        let body = self.inner.recv_body(buf)?;
        let completer = WriteCompleter::new(self.inner);
        Ok((body, completer))
    }

    pub fn reply(self, written_len: usize) {
        let mid = self.inner.mid();
        self.inner.discard_and_reply(Message::WriteReply {
            mid,
            len: written_len,
        });
    }

    pub fn reply_error(self, error: ErrorCode) {
        let mid = self.inner.mid();
        self.inner
            .discard_and_reply(Message::ErrorReply { mid, error });
    }
}

pub struct WriteCompleter<C: ChannelRef>(CompleterInner<C>);

impl<C: ChannelRef> WriteCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        let mid = request.mid();
        Self(CompleterInner::new(request.ch, mid))
    }

    pub fn reply(self, written_len: usize) {
        let mid = self.0.mid();
        self.0.reply(Message::WriteReply {
            mid,
            len: written_len,
        });
    }

    pub fn reply_error(self, error: ErrorCode) {
        let mid = self.0.mid();
        self.0.reply(Message::ErrorReply { mid, error });
    }
}

pub struct OpenReply<C: ChannelRef>(ReplyInner<C>);

impl<C: ChannelRef> OpenReply<C> {
    fn new(ch: C, info: MessageInfo) -> Self {
        Self(ReplyInner::new(ch, info))
    }

    pub fn mid(&self) -> MessageId {
        self.0.mid()
    }

    pub fn recv(self) -> Result<OwnedHandle, ErrorCode> {
        self.0.recv_handle()
    }
}

pub struct ReadReply<C: ChannelRef>(ReplyInner<C>);

impl<C: ChannelRef> ReadReply<C> {
    fn new(ch: C, info: MessageInfo) -> Self {
        Self(ReplyInner::new(ch, info))
    }

    pub fn recv<'a>(self, buf: &'a mut [u8]) -> Result<&'a [u8], ErrorCode> {
        self.0.recv_body(buf)
    }
}

pub struct WriteReply<C: ChannelRef> {
    inner: ReplyInner<C>,
    written_len: usize,
}

impl<C: ChannelRef> WriteReply<C> {
    fn new(ch: C, info: MessageInfo, written_len: usize) -> Self {
        Self {
            inner: ReplyInner::new(ch, info),
            written_len,
        }
    }

    pub fn mid(&self) -> MessageId {
        self.inner.mid()
    }

    pub fn written_len(&self) -> usize {
        self.written_len
    }
}

pub struct ErrorReply<C: ChannelRef> {
    inner: ReplyInner<C>,
    error: ErrorCode,
}

impl<C: ChannelRef> ErrorReply<C> {
    fn new(ch: C, info: MessageInfo, error: ErrorCode) -> Self {
        Self {
            inner: ReplyInner::new(ch, info),
            error,
        }
    }

    pub fn mid(&self) -> MessageId {
        self.inner.mid()
    }

    pub fn error(&self) -> ErrorCode {
        self.error
    }
}

pub enum Incoming<C: ChannelRef> {
    Open(OpenRequest<C>),
    Read(ReadRequest<C>),
    Write(WriteRequest<C>),
    GetAttr {},
    SetAttr {},
    ErrorReply(ErrorReply<C>),
    OpenReply(OpenReply<C>),
    ReadReply(ReadReply<C>),
    WriteReply(WriteReply<C>),
    GetAttrReply {},
    SetAttrReply {},
    Unknown {},
}

pub struct Channel {
    handle: OwnedHandle,
}

impl Channel {
    pub fn new() -> Result<(Channel, Channel), ErrorCode> {
        let (handle0, handle1) = sys_channel_create()?;
        let ch0 = Channel::from_handle(handle0);
        let ch1 = Channel::from_handle(handle1);
        Ok((ch0, ch1))
    }

    pub const fn from_handle(handle: OwnedHandle) -> Self {
        Self { handle }
    }

    pub fn send(&self, message: Message) -> Result<(), ErrorCode> {
        match message {
            Message::Open { mid, path, options } => {
                self.send_body(MessageKind::OPEN, mid, path, options.as_usize())
            }
            Message::Read { mid, offset, len } => {
                self.send_args(MessageKind::READ, mid, offset, len)
            }
            Message::Write { mid, offset, buf } => {
                self.send_body(MessageKind::WRITE, mid, buf, offset)
            }
            Message::Getattr { mid, attr } => {
                self.send_args(MessageKind::GETATTR, mid, attr.as_usize(), 0)
            }
            Message::Setattr { mid, attr, buf } => {
                self.send_body(MessageKind::SETATTR, mid, buf, attr.as_usize())
            }
            Message::ErrorReply { mid, error } => {
                self.send_args(MessageKind::ERROR_REPLY, mid, error.as_usize(), 0)
            }
            Message::OpenReply { mid, handle } => {
                self.send_handle(MessageKind::OPEN_REPLY, mid, handle)
            }
            Message::ReadReply { mid, buf } => self.send_body(MessageKind::READ_REPLY, mid, buf, 0),
            Message::WriteReply { mid, len } => {
                self.send_args(MessageKind::WRITE_REPLY, mid, len, 0)
            }
            Message::GetattrReply { mid, buf } => {
                self.send_body(MessageKind::GETATTR_REPLY, mid, buf, 0)
            }
            Message::SetattrReply { mid, len } => {
                self.send_args(MessageKind::SETATTR_REPLY, mid, len, 0)
            }
        }
    }

    pub fn discard(&self, info: MessageInfo) -> Result<(), ErrorCode> {
        sys_channel_discard(self.handle.id(), info)?;
        Ok(())
    }

    fn send_args(
        &self,
        kind: MessageKind,
        mid: MessageId,
        arg1: usize,
        arg2: usize,
    ) -> Result<(), ErrorCode> {
        let info = MessageInfo::new(kind, mid, 0);
        debug_assert!(!info.has_body() && !info.has_handle());

        sys_channel_send(self.handle.id(), info, arg1, null(), arg2)?;
        Ok(())
    }

    fn send_body(
        &self,
        kind: MessageKind,
        mid: MessageId,
        body: &[u8],
        arg1: usize,
    ) -> Result<(), ErrorCode> {
        let info = MessageInfo::new(kind, mid, body.len());
        debug_assert!(info.has_body() && !info.has_handle());

        sys_channel_send(self.handle.id(), info, arg1, body.as_ptr(), 0)?;
        Ok(())
    }

    fn send_handle(
        &self,
        kind: MessageKind,
        mid: MessageId,
        handle: OwnedHandle,
    ) -> Result<(), ErrorCode> {
        let info = MessageInfo::new(kind, mid, 0);
        debug_assert!(!info.has_body() && info.has_handle());

        let handle_id = handle.id();
        mem::forget(handle);
        sys_channel_send(self.handle.id(), info, 0, null(), handle_id.as_usize())?;
        Ok(())
    }

    pub fn recv_args(&self, info: MessageInfo) -> Result<(), ErrorCode> {
        debug_assert!(!info.has_body() && !info.has_handle());

        sys_channel_recv(self.handle.id(), info, null_mut())?;
        Ok(())
    }

    pub fn recv_body(&self, info: MessageInfo, body: &mut [u8]) -> Result<(), ErrorCode> {
        debug_assert!(info.has_body() && !info.has_handle());
        assert_eq!(body.len(), info.body_len());

        sys_channel_recv(self.handle.id(), info, body.as_mut_ptr())?;
        Ok(())
    }

    pub fn recv_handle(&self, info: MessageInfo) -> Result<OwnedHandle, ErrorCode> {
        debug_assert!(!info.has_body() && info.has_handle());

        let handle_id = sys_channel_recv(self.handle.id(), info, null_mut())?;
        Ok(OwnedHandle::from_raw(handle_id))
    }

    pub fn reply_error(&self, mid: MessageId, error: ErrorCode) {
        if let Err(send_err) = self.send_args(MessageKind::ERROR_REPLY, mid, error.as_usize(), 0) {
            warn!("failed to reply error {:?} to : {:?}", error, send_err);
        }
    }
}

impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Channel")
            .field(&self.handle.id().as_usize())
            .finish()
    }
}

impl Handleable for Channel {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }

    fn into_handle(self) -> OwnedHandle {
        self.handle
    }
}

fn sys_channel_create() -> Result<(OwnedHandle, OwnedHandle), ErrorCode> {
    let mut ids: MaybeUninit<[HandleId; 2]> = MaybeUninit::uninit();
    syscall1(SYS_CHANNEL_CREATE, ids.as_mut_ptr() as usize)?;
    let [id0, id1] = unsafe { ids.assume_init() };
    let handle0 = OwnedHandle::from_raw(id0);
    let handle1 = OwnedHandle::from_raw(id1);
    Ok((handle0, handle1))
}

pub fn sys_channel_send(
    ch: HandleId,
    info: MessageInfo,
    arg1: usize,
    body: *const u8,
    handle_or_arg2: usize,
) -> Result<(), ErrorCode> {
    syscall5(
        SYS_CHANNEL_SEND,
        ch.as_usize(),
        info.as_raw(),
        arg1,
        body as usize,
        handle_or_arg2,
    )?;
    Ok(())
}

pub fn sys_channel_recv(
    ch: HandleId,
    info: MessageInfo,
    body: *mut u8,
) -> Result<HandleId, ErrorCode> {
    let ret = syscall3(
        SYS_CHANNEL_RECV,
        ch.as_usize(),
        info.as_raw(),
        body as usize,
    )?;
    Ok(HandleId::from_raw(ret))
}

pub fn sys_channel_discard(ch: HandleId, info: MessageInfo) -> Result<(), ErrorCode> {
    syscall2(SYS_CHANNEL_DISCARD, ch.as_usize(), info.as_raw())?;
    Ok(())
}
