use core::fmt;
use core::mem::MaybeUninit;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_CHANNEL_CREATE;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall1;

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

    fn from_handle(handle: OwnedHandle) -> Self {
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
}

fn sys_channel_create() -> Result<(OwnedHandle, OwnedHandle), ErrorCode> {
    let mut ids: MaybeUninit<[HandleId; 2]> = MaybeUninit::uninit();
    syscall1(SYS_CHANNEL_CREATE, ids.as_mut_ptr() as usize)?;
    let [id0, id1] = unsafe { ids.assume_init() };
    let handle0 = OwnedHandle::from_raw(id0);
    let handle1 = OwnedHandle::from_raw(id1);
    Ok((handle0, handle1))
}
