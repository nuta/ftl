use alloc::alloc::Layout;
use core::ptr::NonNull;

pub enum AllocError {
    OutOfMemory,
    InvalidLayout(alloc::alloc::LayoutError),
}

pub struct Packet {
    buf: NonNull<u8>,
    head: u16,
    tail: u16,
}

impl Packet {
    pub fn new(capacity: usize) -> Result<Self, AllocError> {
        let layout = Layout::from_size_align(capacity, size_of::<u32>()).map_err(|e| AllocError::InvalidLayout(e))?;

        let buf = unsafe {
            let ptr = alloc::alloc::alloc(layout);
            if ptr.is_null() {
                return Err(AllocError::OutOfMemory);
            }

            NonNull::new_unchecked(ptr)
        };

        Ok(Self { buf, head: 0, tail: 0 })
    }
}
