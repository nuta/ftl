use alloc::alloc::Layout;
use core::{ptr::NonNull, slice};

#[derive(Debug)]
pub enum AllocError {
    OutOfMemory,
    InvalidLayout(alloc::alloc::LayoutError),
}

#[derive(Debug)]
pub enum ReserveError {
    NotAligned,
    BufferTooShort,
}

const BUF_MIN_ALIGN: usize = size_of::<u32>();

pub struct Packet {
    buf: NonNull<u8>,
    capacity: usize,
    head: u16,
    tail: u16,
}

impl Packet {
    pub fn new(capacity: usize) -> Result<Self, AllocError> {
        let layout = Layout::from_size_align(capacity, BUF_MIN_ALIGN)
            .map_err(|e| AllocError::InvalidLayout(e))?;

        let buf = unsafe {
            let ptr = alloc::alloc::alloc(layout);
            if ptr.is_null() {
                return Err(AllocError::OutOfMemory);
            }

            NonNull::new_unchecked(ptr)
        };

        Ok(Self {
            buf,
            capacity,
            head: 0,
            tail: 0,
        })
    }

    fn head(&self) -> usize {
        self.head as usize
    }

    fn tail(&self) -> usize {
        self.tail as usize
    }

    fn head_ptr(&self) -> *const u8 {
        unsafe { self.buf.as_ptr().add(self.head()) }
    }

    pub fn uninit_buf(&self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.buf.as_ptr(), self.capacity) }
    }

    pub fn set_len(&mut self, len: usize) {
        self.head = 0;
        self.tail = len as u16;
    }

    pub fn len(&self) -> usize {
        self.tail() - self.head()
    }

    pub fn read<T>(&mut self) -> Result<&T, ReserveError> {
        assert!(align_of::<T>() <= BUF_MIN_ALIGN);

        let len = size_of::<T>();
        if len > self.len() {
            return Err(ReserveError::BufferTooShort);
        }

        let ptr = self.head_ptr() as *const T;
        if !ptr.is_aligned() {
            return Err(ReserveError::NotAligned);
        }

        // SAFETY: The assertion above guarantees that the length
        //         is in the range of u16.
        self.head += len as u16;

        // SAFETY: The pointer is aligned and the length is checked,
        //         and is alive as long as the `self` is alive.
        Ok(unsafe { &*ptr })
    }
}
