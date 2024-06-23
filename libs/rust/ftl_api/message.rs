use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Mutex;
use core::marker::PhantomData;
use core::mem;
use core::mem::offset_of;
use core::mem::size_of;
use core::mem::MaybeUninit;
use core::ptr;
use core::slice;

use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageBuffer;
use ftl_types::message::MessageInfo;
use ftl_types::message::MESSAGE_DATA_MAX_LEN;

use crate::handle::OwnedHandle;

pub struct BufferGuard<T: MessageType> {
    pool: &'static Mutex<MessageBufferPool>,
    buffer: Box<MessageBuffer>,
    _phantom: PhantomData<T>,
}

static GLOBAL_BUFFER_POOL: Mutex<MessageBufferPool> = Mutex::new(MessageBufferPool::new());

pub struct MessageBufferPool {
    buffer: Vec<Box<MaybeUninit<MessageBuffer>>>,
}

impl MessageBufferPool {
    pub const fn new() -> MessageBufferPool {
        MessageBufferPool { buffer: Vec::new() }
    }

    pub fn use_for_send<T: MessageType>(&mut self, msg: T) -> BufferGuard<T> {
        let dst = self.buffer.as_mut_ptr() as *mut T;
        let src = &msg as *const T;

        // Use ptr::copy_nonoverlapping to avoid calling destructors (i.e. avoid
        // freeing moved handles) and to avoid unnecessary length checks.
        //
        // SAFETY: Let's check the requirements for ptr::copy_nonoverlapping one
        //         by one:
        //
        // > src must be valid for reads of count * size_of::<T>() bytes.
        //
        // True because msg is a valid reference to a single T.
        //
        // > dst must be valid for writes of count * size_of::<T>() bytes.
        //
        // We assume size_of::<T>() <= size_of::<MessageBuffer> holds. It is
        // guaranteed IDL stub generator.
        //
        // > Both src and dst must be properly aligned.
        //
        // True because MessageBuffer is aligned to 16 bytes through #[repr] and
        // IDL stub generator guarantees that it's big enough for all field types
        // in T.
        //
        // > The region of memory beginning at src with a size of
        // > count * size_of::<T>() bytes must not overlap with the region of
        // > memory beginning at dst with the same size.
        //
        // True because self.buffer is unique and we have an exclusive acces
        //  (&mut self) to it.
        unsafe {
            ptr::copy_nonoverlapping::<T>(src, dst, 1);
        }

        // Don't call destructors on handles transferred to this buffer.
        mem::forget(msg);

        // SAFETY: We just wrote to the buffer. Some of the fields are not
        // initialized, but it's fine until the caller use T::MSGINFO.
        let buffer = unsafe { self.buffer.assume_init_ref() };

        BufferGuard {
            buffer,
            _phantom: PhantomData,
        }
    }
}

/// Invariant: size_of::<T>() <= size_of::<MessageBuffer>.
pub trait MessageType {
    const NUM_HANDLES: usize;
    const MSGINFO: MessageInfo;
}

#[repr(C)]
pub struct FsOpenMessage {
    pub path: isize,
    pub handle: OwnedHandle,
}

impl MessageType for FsOpenMessage {
    const NUM_HANDLES: usize = 1;
    const MSGINFO: MessageInfo = MessageInfo::from_raw(0x5a5a5a);
}

impl crate::channel::Channel {
    fn typed_send<T: MessageType>(&self, msg: T) -> Result<(), FtlError> {
        let mut buffer = MessageBufferPool::allocate::<T>();
        let guard = buffer.use_for_send(msg);
        self.send(T::MSGINFO, guard.buffer)
    }
}

#[no_mangle]
pub fn message_buffer_test(ch: crate::channel::Channel) -> Result<(), ftl_types::error::FtlError> {
    ch.typed_send(FsOpenMessage {
        handle: OwnedHandle::from_raw(HandleId::from_raw(1)),
        path: 0x1234abcd,
    })
}
