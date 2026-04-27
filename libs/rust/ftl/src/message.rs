use ftl_types::channel::Attr;
use ftl_types::channel::MessageId;
use ftl_types::channel::OpenOptions;
use ftl_types::channel::Peek;

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

pub enum Incoming<C: AsRef<Channel>> {
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
    Unknown(UnknownMessage<C>),
}

impl<C: AsRef<Channel>> Incoming<C> {
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
            _ => Incoming::Unknown(UnknownMessage::new(ch, peek)),
        }
    }
}

struct RequestInner<C: AsRef<Channel>> {
    ch: C,
    info: MessageInfo,
    handled: bool,
}

impl<C: AsRef<Channel>> RequestInner<C> {
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

        // SAFETY: If body is not large enough, the syscall will fail.
        Ok(&body[..self.info.body_len()])
    }

    /// Sends a reply message to the channel.
    ///
    /// The caller must ensure that the request message has been received
    /// or discarded before calling this method.
    fn reply<'a>(mut self, f: impl FnOnce(MessageId) -> Message<'a>) {
        self.handled = true;
        let m = f(self.mid());
        if let Err(error) = self.ch.as_ref().send(m) {
            debug!("failed to reply: {:?}", error);
        }
    }

    fn reply_error(self, error: ErrorCode) {
        self.reply(|mid| Message::ErrorReply { mid, error });
    }

    fn discard_and_reply<'a>(mut self, f: impl FnOnce(MessageId) -> Message<'a>) {
        self.do_discard_and_reply(f(self.mid()));
    }

    fn discard_and_reply_error(self, error: ErrorCode) {
        self.discard_and_reply(|mid| Message::ErrorReply { mid, error });
    }

    fn do_discard_and_reply(&mut self, m: Message) {
        self.handled = true;

        if let Err(error) = self.ch.as_ref().discard(self.info) {
            debug!("failed to discard before reply: {:?}", error);
        }

        if let Err(error) = self.ch.as_ref().send(m) {
            debug!("failed to reply: {:?}", error);
        }
    }
}

impl<C: AsRef<Channel>> Drop for RequestInner<C> {
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

pub struct RecvError<C: AsRef<Channel>> {
    inner: RequestInner<C>,
    error: ErrorCode,
}

impl<C: AsRef<Channel>> RecvError<C> {
    fn new(inner: RequestInner<C>, error: ErrorCode) -> Self {
        Self { inner, error }
    }

    pub fn error(&self) -> ErrorCode {
        self.error
    }

    pub fn reply_error(self, error: ErrorCode) {
        // FIXME: Guarantee that the message is still in the queue if recv fails.
        self.inner.discard_and_reply_error(error);
    }
}

struct ReplyInner<C: AsRef<Channel>> {
    ch: C,
    info: MessageInfo,
    received: bool,
}

impl<C: AsRef<Channel>> ReplyInner<C> {
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

        // SAFETY: If body is not large enough, the syscall will fail.
        Ok(&body[..self.info.body_len()])
    }
}

impl<C: AsRef<Channel>> Drop for ReplyInner<C> {
    fn drop(&mut self) {
        if self.received {
            return;
        }

        if let Err(error) = self.ch.as_ref().discard(self.info) {
            debug!("failed to discard reply message: {:?}", error);
        }
    }
}

pub struct OpenRequest<C: AsRef<Channel>> {
    options: OpenOptions,
    inner: RequestInner<C>,
}

impl<C: AsRef<Channel>> OpenRequest<C> {
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

    pub fn recv<'a>(
        self,
        path: &'a mut [u8],
    ) -> Result<(&'a [u8], OpenCompleter<C>), RecvError<C>> {
        match self.inner.recv_body(path) {
            Ok(body) => {
                let completer = OpenCompleter::new(self.inner);
                Ok((body, completer))
            }
            Err(error) => Err(RecvError::new(self.inner, error)),
        }
    }

    pub fn reply(self, handle: OwnedHandle) {
        self.inner
            .discard_and_reply(|mid| Message::OpenReply { mid, handle });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.inner.discard_and_reply_error(error);
    }
}

pub struct OpenCompleter<C: AsRef<Channel>>(RequestInner<C>);

impl<C: AsRef<Channel>> OpenCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, handle: OwnedHandle) {
        self.0.reply(|mid| Message::OpenReply { mid, handle });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.0.reply_error(error);
    }
}

pub struct OpenReply<C: AsRef<Channel>>(ReplyInner<C>);

impl<C: AsRef<Channel>> OpenReply<C> {
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

pub struct ReadRequest<C: AsRef<Channel>> {
    offset: usize,
    len: usize,
    inner: RequestInner<C>,
}

impl<C: AsRef<Channel>> ReadRequest<C> {
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

    pub fn recv(self) -> Result<ReadCompleter<C>, RecvError<C>> {
        match self.inner.recv_args() {
            Ok(()) => {
                let completer = ReadCompleter::new(self.inner);
                Ok(completer)
            }
            Err(error) => Err(RecvError::new(self.inner, error)),
        }
    }

    pub fn reply(self, buf: &[u8]) {
        self.inner
            .discard_and_reply(|mid| Message::ReadReply { mid, buf });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.inner.discard_and_reply_error(error);
    }
}

pub struct ReadCompleter<C: AsRef<Channel>>(RequestInner<C>);

impl<C: AsRef<Channel>> ReadCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, buf: &[u8]) {
        self.0.reply(|mid| Message::ReadReply { mid, buf });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.0.reply_error(error);
    }
}

pub struct ReadReply<C: AsRef<Channel>> {
    inner: ReplyInner<C>,
}

impl<C: AsRef<Channel>> ReadReply<C> {
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

pub struct WriteRequest<C: AsRef<Channel>> {
    offset: usize,
    inner: RequestInner<C>,
}

impl<C: AsRef<Channel>> WriteRequest<C> {
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

    pub fn recv<'a>(
        self,
        buf: &'a mut [u8],
    ) -> Result<(&'a [u8], WriteCompleter<C>), RecvError<C>> {
        match self.inner.recv_body(buf) {
            Ok(body) => {
                let completer = WriteCompleter::new(self.inner);
                Ok((body, completer))
            }
            Err(error) => Err(RecvError::new(self.inner, error)),
        }
    }

    pub fn reply(self, written_len: usize) {
        self.inner.discard_and_reply(|mid| {
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

pub struct WriteCompleter<C: AsRef<Channel>>(RequestInner<C>);

impl<C: AsRef<Channel>> WriteCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, written_len: usize) {
        self.0.reply(|mid| {
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

pub struct WriteReply<C: AsRef<Channel>> {
    inner: ReplyInner<C>,
    written_len: usize,
}

impl<C: AsRef<Channel>> WriteReply<C> {
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

pub struct GetAttrRequest<C: AsRef<Channel>> {
    attr: Attr,
    inner: RequestInner<C>,
}

impl<C: AsRef<Channel>> GetAttrRequest<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: RequestInner::new(ch, peek.info),
            attr: Attr::from_usize(peek.arg1),
        }
    }

    pub fn attr(&self) -> Attr {
        self.attr
    }

    pub fn recv(self) -> Result<GetAttrCompleter<C>, RecvError<C>> {
        match self.inner.recv_args() {
            Ok(()) => {
                let completer = GetAttrCompleter::new(self.inner);
                Ok(completer)
            }
            Err(error) => Err(RecvError::new(self.inner, error)),
        }
    }

    pub fn reply(self, buf: &[u8]) {
        self.inner
            .discard_and_reply(|mid| Message::GetAttrReply { mid, buf });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.inner.discard_and_reply_error(error);
    }
}

pub struct GetAttrCompleter<C: AsRef<Channel>>(RequestInner<C>);

impl<C: AsRef<Channel>> GetAttrCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, buf: &[u8]) {
        self.0.reply(|mid| Message::GetAttrReply { mid, buf });
    }

    pub fn reply_error(self, error: ErrorCode) {
        self.0.reply_error(error);
    }
}

pub struct GetAttrReply<C: AsRef<Channel>> {
    inner: ReplyInner<C>,
}

impl<C: AsRef<Channel>> GetAttrReply<C> {
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

pub struct SetAttrRequest<C: AsRef<Channel>> {
    attr: Attr,
    inner: RequestInner<C>,
}

impl<C: AsRef<Channel>> SetAttrRequest<C> {
    fn new(ch: C, peek: Peek) -> Self {
        Self {
            inner: RequestInner::new(ch, peek.info),
            attr: Attr::from_usize(peek.arg1),
        }
    }

    pub fn attr(&self) -> Attr {
        self.attr
    }

    pub fn recv<'a>(
        self,
        buf: &'a mut [u8],
    ) -> Result<(&'a [u8], SetAttrCompleter<C>), RecvError<C>> {
        match self.inner.recv_body(buf) {
            Ok(body) => {
                let completer = SetAttrCompleter::new(self.inner);
                Ok((body, completer))
            }
            Err(error) => Err(RecvError::new(self.inner, error)),
        }
    }

    pub fn reply(self, written_len: usize) {
        self.inner.discard_and_reply(|mid| {
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

pub struct SetAttrCompleter<C: AsRef<Channel>>(RequestInner<C>);

impl<C: AsRef<Channel>> SetAttrCompleter<C> {
    fn new(request: RequestInner<C>) -> Self {
        Self(request)
    }

    pub fn reply(self, written_len: usize) {
        self.0.reply(|mid| {
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

pub struct SetAttrReply<C: AsRef<Channel>> {
    inner: ReplyInner<C>,
    written_len: usize,
}

impl<C: AsRef<Channel>> SetAttrReply<C> {
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

pub struct ErrorReply<C: AsRef<Channel>> {
    inner: ReplyInner<C>,
    error: ErrorCode,
}

impl<C: AsRef<Channel>> ErrorReply<C> {
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

pub struct UnknownMessage<C: AsRef<Channel>> {
    ch: C,
    info: MessageInfo,
}

impl<C: AsRef<Channel>> UnknownMessage<C> {
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

impl<C: AsRef<Channel>> Drop for UnknownMessage<C> {
    fn drop(&mut self) {
        if let Err(error) = self.ch.as_ref().discard(self.info) {
            debug!("failed to discard unknown message: {:?}", error);
        }
    }
}
