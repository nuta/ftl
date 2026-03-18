use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::fmt;
use core::mem;
use core::mem::MaybeUninit;

pub use ftl_types::channel::Attr;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::MessageKind;
pub use ftl_types::channel::OpenOptions;
use ftl_types::channel::RequestId;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_RECV;
use ftl_types::syscall::SYS_CHANNEL_SEND;
use log::warn;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall1;
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

    pub fn send_open(
        &self,
        rid: RequestId,
        path: &[u8],
        options: OpenOptions,
    ) -> Result<(), ErrorCode> {
        let info = MessageInfo::new(MessageKind::OPEN, rid, path.len());
        self.send_with_body(info, options.as_usize(), path)
    }

    pub fn send_read(&self, rid: RequestId, offset: usize, len: usize) -> Result<(), ErrorCode> {
        let info = MessageInfo::new(MessageKind::READ, rid, 0);
        self.send(info, offset)
    }

    pub fn send_write(&self, rid: RequestId, buf: &[u8], offset: usize) -> Result<(), ErrorCode> {
        let info = MessageInfo::new(MessageKind::WRITE, rid, buf.len());
        self.send_with_body(info, offset, buf)
    }

    pub fn reply_read(&self, rid: RequestId, buf: &[u8]) -> Result<(), ErrorCode> {
        let info = MessageInfo::new(MessageKind::READ_REPLY, rid, buf.len());
        self.send_with_body(info, 0, buf)
    }

    pub fn reply_write(&self, rid: RequestId, written_len: usize) -> Result<(), ErrorCode> {
        let info = MessageInfo::new(MessageKind::WRITE_REPLY, rid, 0);
        self.send(info, written_len)
    }

    fn send(&self, info: MessageInfo, arg: usize) -> Result<(), ErrorCode> {
        debug_assert!(!info.has_body() && !info.has_handle());
        sys_channel_send(self.handle.id(), info, arg, None, None)?;
        Ok(())
    }

    fn send_with_body(&self, info: MessageInfo, arg: usize, body: &[u8]) -> Result<(), ErrorCode> {
        debug_assert!(info.has_body() && !info.has_handle());
        sys_channel_send(self.handle.id(), info, arg, Some(body), None)?;
        Ok(())
    }

    fn send_with_handle(
        &self,
        info: MessageInfo,
        arg: usize,
        handle: OwnedHandle,
    ) -> Result<(), ErrorCode> {
        debug_assert!(!info.has_body() && info.has_handle());

        sys_channel_send(self.handle.id(), info, arg, None, Some(handle.id()))?;
        Ok(())
    }

    fn recv_with_body(&self, info: MessageInfo, body: Option<&mut [u8]>) -> Result<(), ErrorCode> {
        debug_assert!(info.has_body() && !info.has_handle());

        let handle_id = sys_channel_recv(self.handle.id(), info, body)?;
        Ok(())
    }

    fn recv_with_handle(&self, info: MessageInfo) -> Result<OwnedHandle, ErrorCode> {
        debug_assert!(!info.has_body() && info.has_handle());

        let handle_id = sys_channel_recv(self.handle.id(), info, None)?;
        Ok(OwnedHandle::from_raw(handle_id))
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
    arg: usize,
    body: Option<&[u8]>,
    handle: Option<HandleId>,
) -> Result<(), ErrorCode> {
    let body_ptr = body
        .map(|body| {
            debug_assert_eq!(body.len(), info.body_len());
            body.as_ptr() as usize
        })
        .unwrap_or(0);

    let handle_id = handle.map(|handle| handle.as_usize()).unwrap_or(0);
    syscall5(
        SYS_CHANNEL_SEND,
        ch.as_usize(),
        info.as_raw(),
        arg,
        body_ptr,
        handle_id,
    )?;
    Ok(())
}

pub fn sys_channel_recv(
    ch: HandleId,
    info: MessageInfo,
    body: Option<&mut [u8]>,
) -> Result<HandleId, ErrorCode> {
    let body_ptr = body
        .map(|body| {
            debug_assert_eq!(body.len(), info.body_len());
            body.as_mut_ptr() as usize
        })
        .unwrap_or(0);

    let ret = syscall3(SYS_CHANNEL_RECV, ch.as_usize(), info.as_raw(), body_ptr)?;
    Ok(HandleId::from_raw(ret))
}
