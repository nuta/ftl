use core::fmt;

pub use ftl_types::channel::MessageInfo;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;

use crate::buffer::Buffer;
use crate::buffer::BufferMut;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;

pub enum Message<'a> {
    ErrorReply { error: ErrorCode },
    Read { offset: usize, len: usize },
    ReadReply { data: &'a [u8] },
    Write { data: &'a [u8], offset: usize },
    WriteReply { len: usize },
    Open { uri: &'a [u8] },
    OpenReply { ch: Channel },
}

#[derive(Debug)]
pub enum SendError {
    Syscall(ErrorCode),
}

pub struct Channel {
    handle: OwnedHandle,
}

impl Channel {
    pub fn send(&self, msg: Message) -> Result<(), SendError> {
        todo!()
    }

    pub fn recv(
        &self,
        data: &mut [u8],
        handles: &mut [HandleId],
    ) -> Result<MessageInfo, ErrorCode> {
        todo!()
    }
}

fn sys_channel_send(
    ch: HandleId,
    msginfo: MessageInfo,
    data: &[u8],
    handles: &[HandleId],
) -> Result<(), ErrorCode> {
    todo!()
}

fn sys_channel_recv(
    ch: HandleId,
    data: &mut [u8],
    handles: &mut [HandleId],
) -> Result<(), ErrorCode> {
    todo!()
}

impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Channel")
            .field(&self.handle.as_usize())
            .finish()
    }
}

impl Handleable for Channel {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}
