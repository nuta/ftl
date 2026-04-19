use alloc::rc::Rc;
use core::fmt;
use core::mem;
use core::mem::MaybeUninit;
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
pub use crate::message::ErrorReply;
pub use crate::message::GetAttrCompleter;
pub use crate::message::GetAttrReply;
pub use crate::message::Incoming;
pub use crate::message::Message;
pub use crate::message::OpenCompleter;
pub use crate::message::OpenReply;
pub use crate::message::ReadCompleter;
pub use crate::message::ReadReply;
pub use crate::message::SetAttrCompleter;
pub use crate::message::SetAttrReply;
pub use crate::message::Unknown;
pub use crate::message::WriteCompleter;
pub use crate::message::WriteReply;
use crate::syscall::syscall1;
use crate::syscall::syscall2;
use crate::syscall::syscall3;
use crate::syscall::syscall5;

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
            Message::GetAttrReply { mid, buf } => {
                self.send_body(MessageKind::GETATTR_REPLY, mid, buf, 0)
            }
            Message::SetAttrReply { mid, len } => {
                self.send_args(MessageKind::SETATTR_REPLY, mid, len, 0)
            }
        }
    }

    pub(crate) fn discard(&self, info: MessageInfo) -> Result<(), ErrorCode> {
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
        // TODO: Close the handle if it fails

        Ok(())
    }

    pub(crate) fn recv_args(&self, info: MessageInfo) -> Result<(), ErrorCode> {
        debug_assert!(!info.has_body() && !info.has_handle());

        sys_channel_recv(self.handle.id(), info, null_mut())?;
        Ok(())
    }

    pub(crate) fn recv_body(&self, info: MessageInfo, body: &mut [u8]) -> Result<(), ErrorCode> {
        debug_assert!(info.has_body() && !info.has_handle());
        assert_eq!(body.len(), info.body_len());

        sys_channel_recv(self.handle.id(), info, body.as_mut_ptr())?;
        Ok(())
    }

    pub(crate) fn recv_handle(&self, info: MessageInfo) -> Result<OwnedHandle, ErrorCode> {
        debug_assert!(!info.has_body() && info.has_handle());

        let handle_id = sys_channel_recv(self.handle.id(), info, null_mut())?;
        Ok(OwnedHandle::from_raw(handle_id))
    }

    pub(crate) fn reply_error(&self, mid: MessageId, error: ErrorCode) {
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
