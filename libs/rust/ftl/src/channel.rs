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
use ftl_types::syscall::SYS_CHANNEL_BODY_READ;
use ftl_types::syscall::SYS_CHANNEL_BODY_WRITE;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_SEND;
use log::warn;

use crate::eventloop::Event;
use crate::eventloop::EventLoop;
use crate::eventloop::Reply;
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

    pub fn connect(name: &str) -> Result<Self, ErrorCode> {
        open_via_bootstrap(format!("connect:{}", name))
    }

    pub fn register(name: &str) -> Result<Self, ErrorCode> {
        open_via_bootstrap(format!("register:{}", name))
    }

    pub(crate) fn call(
        &self,
        info: MessageInfo,
        cookie: usize,
        inline: usize,
        body_addr: usize,
        body_len: usize,
    ) -> Result<(), ErrorCode> {
        sys_channel_send(self.handle.id(), info, cookie, inline, body_addr, body_len)
    }

    pub(crate) fn reply(
        &self,
        info: MessageInfo,
        request_id: RequestId,
        inline: usize,
        handle: Option<HandleId>,
    ) -> Result<(), ErrorCode> {
        sys_channel_send(
            self.handle.id(),
            info,
            request_id.as_u32() as usize,
            inline,
            handle.map(|id| id.as_usize()).unwrap_or(0),
            0,
        )
    }

    pub(crate) fn read_body(
        &self,
        request_id: RequestId,
        offset: usize,
        buf: &mut [u8],
    ) -> Result<usize, ErrorCode> {
        sys_channel_body_read(self.handle.id(), request_id, offset, buf)
    }

    pub(crate) fn write_body(
        &self,
        request_id: RequestId,
        offset: usize,
        buf: &[u8],
    ) -> Result<usize, ErrorCode> {
        sys_channel_body_write(self.handle.id(), request_id, offset, buf)
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
    syscall5(
        SYS_CHANNEL_SEND,
        ch.as_usize(),
        info.as_usize() | (body_len << 8),
        rid_or_cookie,
        inline,
        body_or_handle,
    )?;
    Ok(())
}

pub fn sys_channel_body_read(
    ch: HandleId,
    request_id: RequestId,
    offset: usize,
    buf: &mut [u8],
) -> Result<usize, ErrorCode> {
    syscall5(
        SYS_CHANNEL_BODY_READ,
        ch.as_usize(),
        request_id.as_u32() as usize,
        offset,
        buf.as_ptr() as usize,
        buf.len(),
    )
}

pub fn sys_channel_body_write(
    ch: HandleId,
    request_id: RequestId,
    offset: usize,
    buf: &[u8],
) -> Result<usize, ErrorCode> {
    syscall5(
        SYS_CHANNEL_BODY_WRITE,
        ch.as_usize(),
        request_id.as_u32() as usize,
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
    client.open(uri, ()).unwrap();

    // FIXME:
    mem::forget(bootstrap_ch);

    loop {
        match eventloop.wait() {
            Event::Reply {
                reply: Reply::Open { new_ch, .. },
                ..
            } => {
                return Ok(new_ch);
            }
            Event::Reply {
                reply: Reply::Error { error, .. },
                ..
            } => {
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
