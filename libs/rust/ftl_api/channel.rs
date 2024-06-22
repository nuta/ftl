use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageBuffer;
use ftl_types::message::MessageInfo;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall;

pub struct Channel {
    handle: OwnedHandle,
}

impl Channel {
    pub fn create() -> Result<(Channel, Channel), FtlError> {
        let (handle0, handle1) = syscall::channel_create()?;
        let ch0 = Channel {
            handle: OwnedHandle::from_raw(handle0),
        };
        let ch1 = Channel {
            handle: OwnedHandle::from_raw(handle1),
        };
        Ok((ch0, ch1))
    }

    pub fn send(&self, msginfo: MessageInfo, message: &MessageBuffer) -> Result<(), FtlError> {
        syscall::channel_send(self.handle.id(), msginfo, message as *const _ as *const u8)?;
        Ok(())
    }
}

impl Handleable for Channel {
    fn handle_id(&self) -> HandleId {
        self.handle.id()
    }
}
