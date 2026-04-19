use alloc::rc::Rc;

use ftl_types::channel::Attr;
use ftl_types::channel::MessageId;
use ftl_types::channel::OpenOptions;
use ftl_types::channel::Peek;
use log::debug;

use crate::channel::Channel;
use crate::channel::MessageInfo;
use crate::channel::MessageKind;
use crate::error::ErrorCode;
use crate::handle::OwnedHandle;

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
    GetAttrReply {
        mid: MessageId,
        buf: &'a [u8],
    },
    SetAttrReply {
        mid: MessageId,
        len: usize,
    },
}

pub enum Incoming<C: ChannelRef> {
    Open(OpenRequest<C>),
    Read(ReadRequest<C>),
    Write(WriteRequest<C>),
    GetAttr(GetAttrRequest<C>),
    SetAttr(SetAttrRequest<C>),
    ErrorReply(ErrorReply<C>),
    OpenReply(OpenReply<C>),
    ReadReply(ReadReply<C>),
    WriteReply(WriteReply<C>),
    GetAttrReply(GetAttrReply<C>),
    SetAttrReply(SetAttrReply<C>),
    Unknown(Unknown<C>),
}

impl<C: ChannelRef> Incoming<C> {
    pub fn parse(ch: C, peek: Peek) -> Incoming<C> {
        match peek.info.kind() {
            MessageKind::OPEN => Incoming::Open(OpenRequest::new(ch, peek)),
            MessageKind::READ => Incoming::Read(ReadRequest::new(ch, peek)),
            MessageKind::WRITE => Incoming::Write(WriteRequest::new(ch, peek)),
            MessageKind::GETATTR => Incoming::GetAttr(GetAttrRequest::new(ch, peek)),
            MessageKind::SETATTR => Incoming::SetAttr(SetAttrRequest::new(ch, peek)),
            MessageKind::ERROR_REPLY => Incoming::ErrorReply(ErrorReply::new(ch, peek)),
            MessageKind::OPEN_REPLY => Incoming::OpenReply(OpenReply::new(ch, peek)),
            MessageKind::READ_REPLY => Incoming::ReadReply(ReadReply::new(ch, peek)),
            MessageKind::WRITE_REPLY => Incoming::WriteReply(WriteReply::new(ch, peek)),
            MessageKind::GETATTR_REPLY => Incoming::GetAttrReply(GetAttrReply::new(ch, peek)),
            MessageKind::SETATTR_REPLY => Incoming::SetAttrReply(SetAttrReply::new(ch, peek)),
            _ => Incoming::Unknown(Unknown::new(ch, peek)),
        }
    }
}

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
    handled: bool,
}

impl<C: ChannelRef> RequestInner<C> {
    fn new(ch: C, info: MessageInfo) -> Self {
        Self {
            ch,
            info,
            handled: false,
        }
    }

    fn mid(&self) -> MessageId {
        self.info.mid()
    }

    fn recv_args(&self) -> Result<(), ErrorCode> {
        self.ch.as_ref().recv_args(self.info)?;
        Ok(())
    }

    fn recv_body<'a>(&self, body: &'a mut [u8]) -> Result<&'a [u8], ErrorCode> {
        self.ch.as_ref().recv_body(self.info, body)?;
        Ok(&body[..])
    }

    /// Sends a reply message to the channel.
    ///
    /// The caller must ensure that the request message has been received
    /// or discarded before calling this method.
    fn reply(mut self, m: Message) {
        self.handled = true;
        if let Err(error) = self.ch.as_ref().send(m) {
            debug!("failed to reply: {:?}", error);
        }
    }

    fn reply_with<'a>(self, f: impl FnOnce(MessageId) -> Message<'a>) {
        let mid = self.mid();
        self.reply(f(mid));
    }

    fn discard_and_reply(mut self, m: Message) {
        self.handled = true;
        self.do_discard_and_reply(m);
    }

    fn discard_and_reply_with<'a>(self, f: impl FnOnce(MessageId) -> Message<'a>) {
        let mid = self.mid();
        self.discard_and_reply(f(mid));
    }

    fn reply_error(self, error: ErrorCode) {
        self.reply_with(|mid| Message::ErrorReply { mid, error });
    }

    fn discard_and_reply_error(self, error: ErrorCode) {
        self.discard_and_reply_with(|mid| Message::ErrorReply { mid, error });
    }

    fn do_discard_and_reply(&self, m: Message) {
        if let Err(error) = self.ch.as_ref().discard(self.info) {
            debug!("failed to discard before reply: {:?}", error);
        }

        if let Err(error) = self.ch.as_ref().send(m) {
            debug!("failed to reply: {:?}", error);
        }
    }
}

impl<C: ChannelRef> Drop for RequestInner<C> {
    fn drop(&mut self) {
        if self.handled {
            return;
        }

        debug!("request dropped without reply");

        self.do_discard_and_reply(Message::ErrorReply {
            mid: self.mid(),
            error: ErrorCode::NotHandled,
        });
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

    fn body_len(&self) -> usize {
        self.info.body_len()
    }

    fn recv_handle(mut self) -> Result<OwnedHandle, ErrorCode> {
        self.received = true;
        self.ch.as_ref().recv_handle(self.info)
    }

    fn recv_body<'a>(mut self, body: &'a mut [u8]) -> Result<&'a [u8], ErrorCode> {
        self.received = true;
        self.ch.as_ref().recv_body(self.info, body)?;
        Ok(&body[..])
    }
}

impl<C: ChannelRef> Drop for ReplyInner<C> {
    fn drop(&mut self) {
        if self.received {
            return;
        }

        if let Err(error) = self.ch.as_ref().discard(self.info) {
            debug!("failed to discard reply message: {:?}", error);
        }
    }
}

pub struct OpenRequest<C: ChannelRef> {
    options: OpenOptions,
    inner: RequestInner<C>,
}

impl<C: ChannelRef> OpenRequest<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: RequestInner::new(ch, peek.info),
            options: OpenOptions::from_usize(peek.arg1),
        }
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
        self.inner
            .discard_and_reply_with(|mid| Message::OpenReply { mid, handle });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.inner.discard_and_reply_error(error);
    }
}

pub struct OpenCompleter<C: ChannelRef>(RequestInner<C>);

impl<C: ChannelRef> OpenCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, handle: OwnedHandle) {
        self.0.reply_with(|mid| Message::OpenReply { mid, handle });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.0.reply_error(error);
    }
}

pub struct OpenReply<C: ChannelRef>(ReplyInner<C>);

impl<C: ChannelRef> OpenReply<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self(ReplyInner::new(ch, peek.info))
    }

    pub fn mid(&self) -> MessageId {
        self.0.mid()
    }

    pub fn recv(self) -> Result<OwnedHandle, ErrorCode> {
        self.0.recv_handle()
    }
}

pub struct ReadRequest<C: ChannelRef> {
    offset: usize,
    len: usize,
    inner: RequestInner<C>,
}

impl<C: ChannelRef> ReadRequest<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: RequestInner::new(ch, peek.info),
            offset: peek.arg1,
            len: peek.arg2,
        }
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
        self.inner
            .discard_and_reply_with(|mid| Message::ReadReply { mid, buf });
    }
}

pub struct ReadCompleter<C: ChannelRef>(RequestInner<C>);

impl<C: ChannelRef> ReadCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, buf: &[u8]) {
        self.0.reply_with(|mid| Message::ReadReply { mid, buf });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.0.reply_error(error);
    }
}

pub struct ReadReply<C: ChannelRef> {
    inner: ReplyInner<C>,
}

impl<C: ChannelRef> ReadReply<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: ReplyInner::new(ch, peek.info),
        }
    }

    pub fn mid(&self) -> MessageId {
        self.inner.mid()
    }

    pub fn read_len(&self) -> usize {
        self.inner.body_len()
    }

    pub fn recv<'a>(self, buf: &'a mut [u8]) -> Result<&'a [u8], ErrorCode> {
        self.inner.recv_body(buf)
    }
}

pub struct WriteRequest<C: ChannelRef> {
    offset: usize,
    inner: RequestInner<C>,
}

impl<C: ChannelRef> WriteRequest<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: RequestInner::new(ch, peek.info),
            offset: peek.arg1,
        }
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
        self.inner.discard_and_reply_with(|mid| {
            Message::WriteReply {
                mid,
                len: written_len,
            }
        });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.inner.discard_and_reply_error(error);
    }
}

pub struct WriteCompleter<C: ChannelRef>(RequestInner<C>);

impl<C: ChannelRef> WriteCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, written_len: usize) {
        self.0.reply_with(|mid| {
            Message::WriteReply {
                mid,
                len: written_len,
            }
        });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.0.reply_error(error);
    }
}

pub struct WriteReply<C: ChannelRef> {
    inner: ReplyInner<C>,
    written_len: usize,
}

impl<C: ChannelRef> WriteReply<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: ReplyInner::new(ch, peek.info),
            written_len: peek.arg1,
        }
    }

    pub fn mid(&self) -> MessageId {
        self.inner.mid()
    }

    pub fn written_len(&self) -> usize {
        self.written_len
    }
}

pub struct GetAttrRequest<C: ChannelRef> {
    attr: Attr,
    inner: RequestInner<C>,
}

impl<C: ChannelRef> GetAttrRequest<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: RequestInner::new(ch, peek.info),
            attr: Attr::from_usize(peek.arg1),
        }
    }

    pub fn attr(&self) -> Attr {
        self.attr
    }

    pub fn recv(self) -> Result<GetAttrCompleter<C>, ErrorCode> {
        self.inner.recv_args()?;
        let completer = GetAttrCompleter::new(self.inner);
        Ok(completer)
    }

    pub fn reply(self, buf: &[u8]) {
        self.inner
            .discard_and_reply_with(|mid| Message::GetAttrReply { mid, buf });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.inner.discard_and_reply_error(error);
    }
}

pub struct GetAttrCompleter<C: ChannelRef>(RequestInner<C>);

impl<C: ChannelRef> GetAttrCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, buf: &[u8]) {
        self.0.reply_with(|mid| Message::GetAttrReply { mid, buf });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.0.reply_error(error);
    }
}

pub struct GetAttrReply<C: ChannelRef> {
    inner: ReplyInner<C>,
}

impl<C: ChannelRef> GetAttrReply<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: ReplyInner::new(ch, peek.info),
        }
    }

    pub fn mid(&self) -> MessageId {
        self.inner.mid()
    }

    pub fn read_len(&self) -> usize {
        self.inner.body_len()
    }

    pub fn recv<'a>(self, buf: &'a mut [u8]) -> Result<&'a [u8], ErrorCode> {
        self.inner.recv_body(buf)
    }
}

pub struct SetAttrRequest<C: ChannelRef> {
    attr: Attr,
    inner: RequestInner<C>,
}

impl<C: ChannelRef> SetAttrRequest<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: RequestInner::new(ch, peek.info),
            attr: Attr::from_usize(peek.arg1),
        }
    }

    pub fn attr(&self) -> Attr {
        self.attr
    }

    pub fn recv<'a>(self, buf: &'a mut [u8]) -> Result<(&'a [u8], SetAttrCompleter<C>), ErrorCode> {
        let body = self.inner.recv_body(buf)?;
        let completer = SetAttrCompleter::new(self.inner);
        Ok((body, completer))
    }

    pub fn reply(self, written_len: usize) {
        self.inner.discard_and_reply_with(|mid| {
            Message::SetAttrReply {
                mid,
                len: written_len,
            }
        });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.inner.discard_and_reply_error(error);
    }
}

pub struct SetAttrCompleter<C: ChannelRef>(RequestInner<C>);

impl<C: ChannelRef> SetAttrCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, written_len: usize) {
        self.0.reply_with(|mid| {
            Message::SetAttrReply {
                mid,
                len: written_len,
            }
        });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.0.reply_error(error);
    }
}

pub struct SetAttrReply<C: ChannelRef> {
    inner: ReplyInner<C>,
    written_len: usize,
}

impl<C: ChannelRef> SetAttrReply<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: ReplyInner::new(ch, peek.info),
            written_len: peek.arg1,
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
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: ReplyInner::new(ch, peek.info),
            error: todo!(),
        }
    }

    pub fn mid(&self) -> MessageId {
        self.inner.mid()
    }

    pub fn error(&self) -> ErrorCode {
        self.error
    }
}

pub struct Unknown<C: ChannelRef> {
    ch: C,
    info: MessageInfo,
}

impl<C: ChannelRef> Unknown<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            ch,
            info: peek.info,
        }
    }

    pub fn info(&self) -> MessageInfo {
        self.info
    }
}

impl<C: ChannelRef> Drop for Unknown<C> {
    fn drop(&mut self) {
        if let Err(error) = self.ch.as_ref().discard(self.info) {
            debug!("failed to discard unknown message: {:?}", error);
        }
    }
}
