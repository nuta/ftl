use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::mem::MaybeUninit;

use ftl_types::channel::CallId;
use ftl_types::channel::MessageBody;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::OutOfLine;
use ftl_types::channel::ReadInline;
use ftl_types::channel::WriteInline;
use ftl_types::channel::WriteReplyInline;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_SEND;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall1;
use crate::syscall::syscall5;

pub enum Buffer {
    Static(&'static [u8]),
    String(String),
    Vec(Vec<u8>),
}

impl Buffer {
    fn to_ool(&self) -> OutOfLine {
        match self {
            Buffer::Static(b) => {
                OutOfLine {
                    addr: b.as_ptr() as usize,
                    len: b.len(),
                }
            }
            Buffer::String(s) => {
                OutOfLine {
                    addr: s.as_ptr() as usize,
                    len: s.len(),
                }
            }
            Buffer::Vec(v) => {
                OutOfLine {
                    addr: v.as_ptr() as usize,
                    len: v.len(),
                }
            }
        }
    }
}

pub enum BufferMut {
    String(String),
    Vec(Vec<u8>),
}

impl BufferMut {
    fn to_ool(&self) -> OutOfLine {
        match self {
            BufferMut::String(s) => {
                OutOfLine {
                    addr: s.as_ptr() as usize,
                    len: s.len(),
                }
            }
            BufferMut::Vec(v) => {
                OutOfLine {
                    addr: v.as_ptr() as usize,
                    len: v.len(),
                }
            }
        }
    }
}

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

pub(crate) enum Cookie {
    Buffer(Buffer),
    BufferMut(BufferMut),
}

/// A reply message to send back to the caller.
pub enum Reply {
    WriteReply {
        /// The number of bytes written.
        len: usize,
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

    fn from_handle(handle: OwnedHandle) -> Self {
        Self { handle }
    }

    /// Create a Channel from a raw handle ID.
    ///
    /// # Safety
    /// The caller must ensure that the handle ID refers to a valid Channel.
    pub unsafe fn from_raw_id(id: HandleId) -> Self {
        Self {
            handle: OwnedHandle::from_raw(id),
        }
    }

    pub fn send(&self, message: Message) -> Result<(), ErrorCode> {
        let body = MaybeUninit::<MessageBody>::uninit();
        // TODO: Double check the safety of this.
        let mut body = unsafe { body.assume_init() };
        let (info, cookie) = match message {
            Message::Open { uri } => {
                body.ools[0] = uri.to_ool();
                (MessageInfo::OPEN, Cookie::Buffer(uri))
            }
            Message::Read { offset, data } => {
                body.ools[0] = data.to_ool();
                // FIXME: Ugly unsafe code. Alignment is not guaranteed.
                let inline = unsafe { &mut *(body.inline.as_mut_ptr() as *mut ReadInline) };
                *inline = ReadInline {
                    offset,
                    len: body.ools[0].len, // TODO: Should we add len field to Message?
                };
                (MessageInfo::READ, Cookie::BufferMut(data))
            }
            Message::Write { offset, data } => {
                body.ools[0] = data.to_ool();
                let inline = unsafe { &mut *(body.inline.as_mut_ptr() as *mut WriteInline) };
                *inline = WriteInline {
                    offset,
                    len: body.ools[0].len,
                };
                (MessageInfo::WRITE, Cookie::Buffer(data))
            }
        };

        let cookie = Box::into_raw(Box::new(cookie)) as usize;
        sys_channel_send(self.handle.id(), info, &body, cookie, CallId::new(0))?;
        Ok(())
    }

    /// Send a reply to a call.
    pub fn reply(&self, call_id: CallId, reply: Reply) -> Result<(), ErrorCode> {
        let body = MaybeUninit::<MessageBody>::uninit();
        let mut body = unsafe { body.assume_init() };

        let info = match reply {
            Reply::WriteReply { len } => {
                let inline = unsafe { &mut *(body.inline.as_mut_ptr() as *mut WriteReplyInline) };
                *inline = WriteReplyInline { len };
                MessageInfo::WRITE_REPLY
            }
        };

        sys_channel_send(self.handle.id(), info, &body, 0, call_id)?;
        Ok(())
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

pub fn sys_channel_send(
    ch: HandleId,
    info: MessageInfo,
    body: &MessageBody,
    cookie: usize,
    call_id: CallId,
) -> Result<(), ErrorCode> {
    syscall5(
        SYS_CHANNEL_SEND,
        ch.as_usize(),
        info.as_u32() as usize,
        body as *const MessageBody as usize,
        cookie,
        call_id.as_u32() as usize,
    )?;
    Ok(())
}
