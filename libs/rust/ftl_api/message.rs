use alloc::boxed::Box;
use ftl_types::handle::HandleId;
use core::mem::offset_of;
use core::mem::size_of;
use core::mem::MaybeUninit;
use core::mem;
use core::slice;

use ftl_types::message::MessageBuffer;
use ftl_types::message::MessageInfo;
use ftl_types::message::MESSAGE_DATA_MAX_LEN;

use crate::handle::OwnedHandle;

pub struct OwnedMessageBuffer {
    buffer: Box<MaybeUninit<MessageBuffer>>,
}

impl OwnedMessageBuffer {
    pub fn allocate<T>() -> OwnedMessageBuffer {
        // Ideally this should be a static_assert!
        debug_assert!(
            MESSAGE_DATA_MAX_LEN < size_of::<T>(),
            "T is too large for MessageBuffer"
        );

        OwnedMessageBuffer {
            buffer: Box::new(MaybeUninit::uninit()),
        }
    }

    pub fn use_for_send<T: MessageType>(&mut self, msg: T) -> &MessageBuffer {
        let buffer_data_addr = self.buffer.as_mut_ptr() as usize;

        // SAFETY: We have mutable (exclusive) access thanks to the &mut self,
        //         and it has exactly the right size for a MessageBuffer.
        let dst = unsafe { slice::from_raw_parts_mut(buffer_data_addr as *mut u8, MESSAGE_DATA_MAX_LEN) };
        // SAFETY: It's just another way to reference the message. Also,
        //         we owns T and don't have any other references to it.
        let src = unsafe { slice::from_raw_parts(&msg as *const T as *const u8, size_of::<T>()) };

        dst.copy_from_slice(src);

        // Don't call destructors on handles transferred to this buffer.
        mem::forget(msg);

        // SAFETY: We just wrote to the buffer. Some of the fields are not
        // initialized, but it's fine until the caller use T::MSGINFO.
        unsafe { self.buffer.assume_init_ref() }
    }
}

trait MessageType {
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

pub fn main(mut buffer: OwnedMessageBuffer, ch: crate::channel::Channel) -> Result<(), ftl_types::error::FtlError> {
    let buf = buffer.use_for_send(FsOpenMessage {
        handle: OwnedHandle::from_raw(HandleId::from_raw(1)),
        path: 0x1234abcd,
    });

    ch.send(FsOpenMessage::MSGINFO, buf)
}
