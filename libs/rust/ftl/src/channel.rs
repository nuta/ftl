use core::fmt;

use ftl_types::channel::MessageBody;
pub use ftl_types::channel::MessageInfo;
use ftl_types::channel::TxId;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;

use crate::application::Cookie;
use crate::buffer::Buffer;
use crate::buffer::BufferMut;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;

/// A message constructor to send to a channel.
pub enum Message {
    Open {
        /// The URI to open.
        uri: Buffer,
    },
    Read {
        /// The offset to read from.
        offset: usize,
        /// The buffer to read into. The receiver will write this buffer up
        /// to the length of this buffer.
        data: BufferMut,
    },
    Write {
        /// The offset to write to.
        offset: usize,
        /// The buffer to write from. The sender will read this buffer up to
        /// the length of this buffer.
        data: Buffer,
    },
}

pub enum Reply {
    OpenReply {
        /// The new channel.
        ch: Channel,
    },
    ReadReply {
        /// The number of bytes actually read.
        len: usize,
    },
    WriteReply {
        /// The number of bytes actually written.
        len: usize,
    },
    ErrorReply {
        /// The error code.
        error: ErrorCode,
    },
}

#[derive(Debug)]
pub enum SendError {
    Syscall(ErrorCode),
}

pub struct Channel {
    handle: OwnedHandle,
}

impl Channel {
    pub fn from_handle(handle: OwnedHandle) -> Self {
        Self { handle }
    }

    pub fn send(&self, msg: Message) -> Result<(), SendError> {
        todo!()
    }

    pub(crate) fn reply(&self, reply: Reply) -> Result<(), SendError> {
        todo!()
    }
}

fn sys_channel_send(
    ch: HandleId,
    info: MessageInfo,
    msg: &MessageBody,
    cookie: Cookie,
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
