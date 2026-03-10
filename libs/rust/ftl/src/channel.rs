use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::mem;
use core::mem::MaybeUninit;

pub use ftl_types::channel::Attr;
use ftl_types::channel::CallId;
use ftl_types::channel::MessageBody;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::OutOfLine;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_OOL_READ;
use ftl_types::syscall::SYS_CHANNEL_OOL_WRITE;
use ftl_types::syscall::SYS_CHANNEL_SEND;
use log::warn;

use crate::eventloop::Event;
use crate::eventloop::EventLoop;
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

    // TODO: Make this private
    pub const fn from_handle(handle: OwnedHandle) -> Self {
        Self { handle }
    }

    pub fn connect(name: &str) -> Result<Self, ErrorCode> {
        open_via_bootstrap(format!("connect:{}", name))
    }

    pub fn register(name: &str) -> Result<Self, ErrorCode> {
        open_via_bootstrap(format!("register:{}", name))
    }

    pub(crate) fn call(
        &self,
        info: MessageInfo,
        body: &MessageBody,
        cookie: usize,
    ) -> Result<(), ErrorCode> {
        sys_channel_send(self.handle.id(), info, body, cookie, CallId::new(0))?;
        Ok(())
    }

    pub(crate) fn reply(
        &self,
        info: MessageInfo,
        body: &MessageBody,
        call_id: CallId,
    ) -> Result<(), ErrorCode> {
        sys_channel_send(self.handle.id(), info, body, 0, call_id)?;
        Ok(())
    }

    pub fn ool_read(
        &self,
        call_id: CallId,
        index: usize,
        offset: usize,
        buf: &mut [u8],
    ) -> Result<usize, ErrorCode> {
        sys_channel_ool_read(self.handle.id(), call_id, index, offset, buf)
    }

    pub fn ool_write(
        &self,
        call_id: CallId,
        index: usize,
        offset: usize,
        buf: &[u8],
    ) -> Result<usize, ErrorCode> {
        sys_channel_ool_write(self.handle.id(), call_id, index, offset, buf)
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

pub fn sys_channel_ool_read(
    ch: HandleId,
    call_id: CallId,
    index: usize,
    offset: usize,
    buf: &mut [u8],
) -> Result<usize, ErrorCode> {
    // FIXME: Define call ID & ool index ranges.
    debug_assert!(index < 16);
    debug_assert!(call_id.as_u32() < 0xfff_ffff);

    syscall5(
        SYS_CHANNEL_OOL_READ,
        ch.as_usize(),
        (call_id.as_u32() as usize) << 4 | index,
        offset,
        buf.as_ptr() as usize,
        buf.len(),
    )
}

pub fn sys_channel_ool_write(
    ch: HandleId,
    call_id: CallId,
    index: usize,
    offset: usize,
    buf: &[u8],
) -> Result<usize, ErrorCode> {
    // FIXME: Define call ID & ool index ranges.
    debug_assert!(index < 16);
    debug_assert!(call_id.as_u32() < 0xfff_ffff);

    syscall5(
        SYS_CHANNEL_OOL_WRITE,
        ch.as_usize(),
        (call_id.as_u32() as usize) << 4 | index,
        offset,
        buf.as_ptr() as usize,
        buf.len(),
    )
}

fn open_via_bootstrap(uri: String) -> Result<Channel, ErrorCode> {
    let bootstrap_ch: Rc<Channel> = Rc::new(Channel::from_handle(OwnedHandle::from_raw(
        HandleId::from_raw(1),
    )));

    let mut eventloop = EventLoop::new().unwrap();
    let client = eventloop.add_channel(bootstrap_ch.clone(), ()).unwrap();
    client.open(Buffer::String(uri), ()).unwrap();

    // FIXME:
    mem::forget(bootstrap_ch);

    loop {
        match eventloop.wait() {
            Event::OpenReply { new_ch, cookie, .. } => {
                return Ok(new_ch);
            }
            Event::ErrorReply { error, .. } => {
                warn!("service discovery failed: {:?}", error);
                return Err(error);
            }
            Event::SinkError(error) => {
                return Err(error);
            }
            event => {
                panic!("unexpected bootstrap event: {:?}", event);
            }
        }
    }
}
