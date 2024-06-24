use core::mem::size_of;

use crate::handle::HandleId;

/// The message metadata.
///
/// # Layout
///
/// ```plain
///
/// |63 or 31                                      14|13   12|11         0|
/// +------------------------------------------------+-------+------------+
/// |                       TYPE                     |   H   |    LEN     |
/// +------------------------------------------------+-------+------------+
///
/// LEN  (12 bits) - # of bytes in the message data.
/// H    (2 bits)  - # of handles in the message.
/// TYPE (rest)    - Message type.
///
/// ```
///
/// # Design decisions
///
/// ## A single `isize` for `TYPE`, `H`, and `LEN`
///
/// This is because:
///
/// - It can fit in a single CPU register. Specifying the message metadata
///   can be done with simply setting an immediate value to a register.
///
/// - The message structure (i.e. its length and the number of handles) can be
///   validated at once when checking the message type.
///
/// - It forcibly limits the message data length and the number of handles, in
///   other words, the kernel just need to do bitwise AND operations to get them,
///   without any additional checks.
///
/// ## Message metadata is not part of the message buffer
///
/// This is because:
///
/// - If it was part of the message buffer, the kernel would need to read the
///   message buffer first to determine the number/length of handles/data that
///   it needs to copy.
///
/// - It's useful for debugging. We can determine the message type even if an
///   app accidentally passed an invalid pointer to the kernel. It could be a
///   key clue for debugging.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct MessageInfo(isize);

impl MessageInfo {
    pub const fn from_raw(raw: isize) -> Self {
        Self(raw)
    }

    pub const fn as_raw(self) -> isize {
        self.0
    }

    pub const fn message_type(self) -> isize {
        self.0 >> 14
    }

    pub const fn num_handles(self) -> usize {
        self.0 as usize >> 12 & 0b11
    }

    pub const fn data_len(self) -> usize {
        // FIXME:
        debug_assert!(self.0 & 0xffff < MESSAGE_DATA_MAX_LEN as isize);

        (self.0 & 0xffff) as usize
    }
}

pub const MESSAGE_DATA_MAX_LEN: usize = 4096 - 4 * size_of::<HandleId>();

#[repr(C, align(16))] // Don't reorder fields
pub struct MessageBuffer {
    pub handles: [HandleId; 4],
    pub data: [u8; MESSAGE_DATA_MAX_LEN],
}
