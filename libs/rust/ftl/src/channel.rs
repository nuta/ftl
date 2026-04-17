use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::ptr::null;
use core::ptr::null_mut;

pub use ftl_types::channel::Attr;
pub use ftl_types::channel::MessageId;
pub use ftl_types::channel::MessageInfo;
pub use ftl_types::channel::MessageKind;
pub use ftl_types::channel::OpenOptions;
use ftl_types::channel::PeekedMessage;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_DISCARD;
use ftl_types::syscall::SYS_CHANNEL_PEEK;
use ftl_types::syscall::SYS_CHANNEL_RECV;
use ftl_types::syscall::SYS_CHANNEL_SEND;
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

pub struct RecvToken<'a, R: ?Sized> {
    ch: &'a Channel,
    info: MessageInfo,
    _pd: PhantomData<R>,
}

impl<'a, R: ?Sized> RecvToken<'a, R> {
    pub fn new(ch: &'a Channel, info: MessageInfo) -> Self {
        Self {
            ch,
            info,
            _pd: PhantomData,
        }
    }

    pub fn mid(&self) -> MessageId {
        self.info.mid()
    }
}

impl<'a> RecvToken<'a, ()> {
    pub fn recv(self) -> Result<(), ErrorCode> {
        self.ch.recv_args(self.info)?;
        Ok(())
    }
}

impl<'a> RecvToken<'a, (usize, usize)> {
    pub fn recv(self) -> Result<(), ErrorCode> {
        self.ch.recv_args(self.info)?;
        Ok(())
    }
}

impl<'a> RecvToken<'a, OwnedHandle> {
    pub fn recv(self) -> Result<OwnedHandle, ErrorCode> {
        let handle = self.ch.recv_handle(self.info)?;
        Ok(handle)
    }
}

impl<'a> RecvToken<'a, [u8]> {
    pub fn recv(self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        self.ch.recv_body(self.info, buf)?;
        Ok(())
    }
}

pub struct DiscardToken<'a> {
    ch: &'a Channel,
    info: MessageInfo,
}

impl<'a> DiscardToken<'a> {
    pub fn new(ch: &'a Channel, info: MessageInfo) -> Self {
        Self { ch, info }
    }

    pub fn discard(self) -> Result<(), ErrorCode> {
        self.ch.discard(self.info)?;
        Ok(())
    }
}


impl<'a> Peek<'a> {
    pub fn parse(ch: &'a Channel, raw: PeekedMessage) -> Peek<'a> {
        match raw.info.kind() {
            MessageKind::OPEN => {
                let recv = RecvToken::new(ch, raw.info);
                let options = OpenOptions::from_usize(raw.arg1);
                let completer = Completer::new(ch, MessageKind::OPEN_REPLY, raw.info.mid());
                Peek::Open { recv, completer, options, path_len: raw.info.body_len() }
            }
            MessageKind::READ => {
                let recv = RecvToken::new(ch, raw.info);
                Peek::Read { recv }
            }
            MessageKind::WRITE => {
                let recv = RecvToken::new(ch, raw.info);
                let completer = Completer::new(ch, MessageKind::WRITE_REPLY, raw.info.mid());
                Peek::Write { recv, len: raw.info.body_len(), completer }
            }
            MessageKind::GETATTR => {
                let recv = RecvToken::new(ch, raw.info);
                let attr = Attr::from_usize(raw.arg1);
                Peek::GetAttr { recv, attr }
            }
            MessageKind::SETATTR => {
                let recv = RecvToken::new(ch, raw.info);
                let attr = Attr::from_usize(raw.arg1);
                Peek::SetAttr { recv, attr }
            }
            MessageKind::ERROR_REPLY => {
                let recv = RecvToken::new(ch, raw.info);
                let error = ErrorCode::from(raw.arg1);
                Peek::ErrorReply { recv, error }
            }
            MessageKind::OPEN_REPLY => {
                let recv = RecvToken::new(ch, raw.info);
                Peek::OpenReply { recv }
            }
            MessageKind::READ_REPLY => {
                let recv = RecvToken::new(ch, raw.info);
                Peek::ReadReply { recv, len: raw.arg1 }
            }
            MessageKind::WRITE_REPLY => {
                let recv = RecvToken::new(ch, raw.info);
                Peek::WriteReply { recv, len: raw.arg1 }
            }
            MessageKind::GETATTR_REPLY => {
                let recv = RecvToken::new(ch, raw.info);
                Peek::GetAttrReply { recv }
            }
            MessageKind::SETATTR_REPLY => {
                let recv = RecvToken::new(ch, raw.info);
                    Peek::SetAttrReply { recv }
            }
            _ => Peek::Unknown { discard: DiscardToken::new(ch, raw.info) },
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
    pub fn new(ch: &'a Channel, reply_kind: MessageKind, mid: MessageId) -> Self {
        Self { ch, reply_kind, mid, _pd: PhantomData }
    }

    pub fn reply_error(self, error: ErrorCode) -> Result<(), ErrorCode> {
        self.ch.send_args(self.reply_kind, self.mid, error.as_usize(), 0)?;
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
    pub fn new(ch: Rc<Channel>, reply_kind: MessageKind, mid: MessageId) -> Self {
        Self { ch, reply_kind, mid, _pd: PhantomData     }
    }

    pub fn reply_error(self, error: ErrorCode) -> Result<(), ErrorCode> {
        self.ch.send_args(self.reply_kind, self.mid, error.as_usize(), 0)?;
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

pub enum Peek<'a> {
    Open {
        recv: RecvToken<'a, [u8]>,
        completer: Completer<'a, OwnedHandle>,
        path_len: usize,
        options: OpenOptions,
    },
    Read {
        recv: RecvToken<'a, (usize, usize)>,
    },
    Write {
        len: usize,
        recv: RecvToken<'a, [u8]>,
        completer: Completer<'a, usize>,
    },
    GetAttr {
        attr: Attr,
        recv: RecvToken<'a, (usize, usize)>,
    },
    SetAttr {
        attr: Attr,
        recv: RecvToken<'a, [u8]>,
    },
    ErrorReply {
        error: ErrorCode,
        recv: RecvToken<'a, ()>,
    },
    OpenReply {
        recv: RecvToken<'a, OwnedHandle>,
    },
    ReadReply {
        len: usize,
        recv: RecvToken<'a, [u8]>,
    },
    WriteReply {
        len: usize,
        recv: RecvToken<'a, ()>,
    },
    GetAttrReply {
        recv: RecvToken<'a, [u8]>,
    },
    SetAttrReply {
        recv: RecvToken<'a, ()>,
    },
    Unknown {
        discard: DiscardToken<'a>,
    },
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

    pub fn peek(&self) -> Result<Peek<'_>, ErrorCode> {
        let raw = sys_channel_peek(self.handle.id())?;
        Ok(Peek::parse(self, raw))
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

pub fn sys_channel_peek(ch: HandleId) -> Result<PeekedMessage, ErrorCode> {
    let mut peek = MaybeUninit::<PeekedMessage>::uninit();
    let ret = syscall2(SYS_CHANNEL_PEEK, ch.as_usize(), peek.as_mut_ptr() as usize)?;

    Ok(unsafe { peek.assume_init() })
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
