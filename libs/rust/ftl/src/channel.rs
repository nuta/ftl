use core::fmt;

use ftl_types::error::ErrorCode;

use crate::buffer::Buffer;
use crate::buffer::BufferMut;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;

pub enum Message {
    Read { buf: BufferMut, offset: usize },
    ReadReply { len: usize },
    Write { buf: Buffer, offset: usize },
    WriteReply { len: usize },
    Open { uri: Buffer },
    OpenReply { channel: OwnedHandle },
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
