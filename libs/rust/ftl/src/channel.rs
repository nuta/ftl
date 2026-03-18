use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::fmt;
use core::mem;
use core::mem::MaybeUninit;

pub use ftl_types::channel::Attr;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::RequestId;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_SEND;
use log::warn;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall1;
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
    rid_or_cookie: usize, // cookie (requests) or request_id (replies)
    inline: usize,
    body_or_handle: usize, // body_ptr (requests) or handle (replies)
    body_len: usize,
) -> Result<(), ErrorCode> {
    todo!()
    // syscall5(
    //     SYS_CHANNEL_SEND,
    //     ch.as_usize(),
    //     info.as_usize() | (body_len << 8),
    //     rid_or_cookie,
    //     inline,
    //     body_or_handle,
    // )?;
    // Ok(())
}
