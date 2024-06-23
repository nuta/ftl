use alloc::boxed::Box;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::ptr;
use core::ptr::NonNull;

use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageBuffer;
use ftl_types::message::MessageInfo;

use crate::handle::OwnedHandle;

pub struct BufferGuard<'a, T: MessageType> {
    pool: &'a mut MessageBufferPool,
    buffer: NonNull<MaybeUninit<MessageBuffer>>,
    _phantom: PhantomData<T>,
}

impl<'a, T: MessageType> BufferGuard<'a, T> {
    pub fn buffer(&self) -> &MessageBuffer {
        // SAFETY: We know that the buffer is initialized because it was
        // initialized by use_for_send.
        unsafe { self.buffer.as_ref().assume_init_ref() }
    }

    pub fn msginfo(&self) -> MessageInfo {
        T::MSGINFO
    }
}

impl<'a, T: MessageType> Drop for BufferGuard<'a, T> {
    fn drop(&mut self) {
        let boxed = unsafe { Box::from_raw(self.buffer.as_ptr()) };

        // Try queueing the buffer for reuse. If it's full, we just free the buffer.
        self.pool.free_buffer = Some(boxed);
    }
}

pub struct MessageBufferPool {
    free_buffer: Option<Box<MaybeUninit<MessageBuffer>>>,
}

impl MessageBufferPool {
    const fn new() -> MessageBufferPool {
        MessageBufferPool { free_buffer: None }
    }

    #[inline(always)]
    pub fn use_for_send<T: MessageType>(&mut self, msg: T) -> BufferGuard<T> {
        let mut buffer = self
            .free_buffer
            .take()
            .unwrap_or_else(|| Box::new(MaybeUninit::uninit()));

        let dst = buffer.as_mut_ptr() as *mut T;
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

        BufferGuard {
            pool: self,
            buffer: unsafe { NonNull::new_unchecked(Box::into_raw(buffer)) },
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
    fn typed_send<T: MessageType>(
        &self,
        pool: &mut MessageBufferPool,
        msg: T,
    ) -> Result<(), FtlError> {
        let guard = pool.use_for_send(msg);
        self.send(guard.msginfo(), guard.buffer())
    }
}

#[no_mangle]
pub fn message_buffer_test(
    ch: crate::channel::Channel,
    pool: &mut MessageBufferPool,
) -> Result<(), ftl_types::error::FtlError> {
    ch.typed_send(
        pool,
        FsOpenMessage {
            handle: OwnedHandle::from_raw(HandleId::from_raw(1)),
            path: 0x1234abcd,
        },
    )
}
