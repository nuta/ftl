use core::mem;
use core::ptr;

use ftl_types::handle::HandleId;
use ftl_types::message::MessageBuffer;
use ftl_types::message::MessageInfo;

/// Invariant: size_of::<T>() <= size_of::<MessageBuffer>.
pub trait MessageType {
    const NUM_HANDLES: usize;
    const MSGINFO: MessageInfo;
}

#[repr(C)]
pub struct FsOpenMessage {
    pub path: isize,
}

impl MessageType for FsOpenMessage {
    const NUM_HANDLES: usize = 1;
    const MSGINFO: MessageInfo = MessageInfo::from_raw(0x5a5a5a);
}

#[inline(always)]
pub fn use_for_send<T: MessageType>(buffer: &mut MessageBuffer, msg: T) {
    let dst = buffer as *mut _ as *mut T;
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
}

extern "C" {
    fn do_send(ch: HandleId, msg: &MessageBuffer, info: MessageInfo);
}

impl crate::channel::Channel {
    #[inline(always)]
    fn typed_send<T: MessageType>(&self, buf: &mut MessageBuffer, msg: T) {
        use_for_send(buf, msg);
        // unsafe { do_send(self.handle().id(), buf, T::MSGINFO) }
        // self.send(T::MSGINFO, buf);
    }
}

#[no_mangle]
pub fn message_buffer_test(ch: crate::channel::Channel, buf: &mut MessageBuffer,) {
    ch.typed_send(buf, FsOpenMessage {
        path: 0x1234abcd,
    });
}
