use alloc::alloc::Layout;
use core::ptr::NonNull;
use core::slice;

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

// TODO: Rename
pub trait WriteableToPacket {}

const HEAD_PAD: usize = 2; // eth frame is 14 bytes
const BUF_MIN_ALIGN: usize = size_of::<u32>();

pub struct Packet {
    buf: NonNull<u8>,
    capacity: usize,
    head: u16,
    tail: u16,
}

impl Packet {
    pub fn new(len: usize, head_room: usize) -> Result<Self, AllocError> {
        debug_assert!(head_room <= u16::MAX as usize);
        debug_assert!(len <= u16::MAX as usize);

        let capacity = len + head_room;
        let layout = Layout::from_size_align(capacity + HEAD_PAD, BUF_MIN_ALIGN)
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
            head: head_room as u16,
            tail: head_room as u16,
        })
    }

    fn head(&self) -> usize {
        self.head as usize
    }

    fn tail(&self) -> usize {
        self.tail as usize
    }

    fn head_ptr(&self) -> *const u8 {
        unsafe { self.buf_ptr().add(self.head()) }
    }

    fn buf_ptr(&self) -> *const u8 {
        unsafe { self.buf.as_ptr().add(HEAD_PAD) }
    }

    fn buf_mut_ptr(&self) -> *mut u8 {
        unsafe { self.buf.as_ptr().add(HEAD_PAD) }
    }

    pub fn uninit_buf(&self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.buf_mut_ptr(), self.capacity) }
    }

    pub fn slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.buf_ptr(), self.len()) }
    }

    // TODO: remove this
    pub fn set_len(&mut self, len: usize) {
        self.head = 0;
        self.tail = len as u16;
    }

    pub fn len(&self) -> usize {
        self.tail() - self.head()
    }

    pub fn read<T>(&mut self) -> Result<&T, ReserveError> {
        info!(
            "reading type: {:?}, align: {:?}",
            core::any::type_name::<T>(),
            align_of::<T>()
        );
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

    pub fn write_front<T: WriteableToPacket>(&mut self, value: T) -> Result<(), ReserveError> {
        let len = size_of::<T>();
        debug_assert!(len <= u16::MAX as usize);

        if len > self.head() {
            return Err(ReserveError::BufferTooShort);
        }

        let new_head = self.head() - len;
        let ptr = unsafe { self.buf_mut_ptr().add(new_head) as *mut T };
        if !ptr.is_aligned() {
            return Err(ReserveError::NotAligned);
        }

        unsafe { ptr.write(value) };

        self.head = new_head as u16;
        Ok(())
    }

    pub fn write_back<T: WriteableToPacket>(&mut self, value: T) -> Result<(), ReserveError> {
        let len = size_of::<T>();
        debug_assert!(len <= u16::MAX as usize);

        if self.tail() + len > self.capacity {
            return Err(ReserveError::BufferTooShort);
        }

        let ptr = unsafe { self.buf_mut_ptr().add(self.tail()) as *mut T };
        if !ptr.is_aligned() {
            return Err(ReserveError::NotAligned);
        }

        unsafe { ptr.write(value) };

        self.tail += len as u16;
        Ok(())
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        // SAFETY: The layout is already validated in the constructor.
        let layout =
            unsafe { Layout::from_size_align_unchecked(self.capacity + HEAD_PAD, BUF_MIN_ALIGN) };

        unsafe {
            alloc::alloc::dealloc(self.buf.as_ptr() as *mut u8, layout);
        }
    }
}
