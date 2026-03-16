use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Deref;

#[derive(Debug)]
pub struct Buffer(bytes::Bytes);

impl Buffer {
    pub(crate) fn addr_and_len(&self) -> (usize, usize) {
        (self.0.as_ptr() as usize, self.0.len())
    }
}

impl Deref for Buffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Into<Buffer> for &'static str {
    fn into(self) -> Buffer {
        Buffer(bytes::Bytes::from(self))
    }
}

impl Into<Buffer> for &'static [u8] {
    fn into(self) -> Buffer {
        Buffer(bytes::Bytes::from(self))
    }
}

impl Into<Buffer> for Vec<u8> {
    fn into(self) -> Buffer {
        Buffer(bytes::Bytes::from(self))
    }
}

impl Into<Buffer> for String {
    fn into(self) -> Buffer {
        Buffer(bytes::Bytes::from(self))
    }
}

impl Into<Buffer> for BufferMut {
    fn into(self) -> Buffer {
        Buffer(self.0.into())
    }
}

#[derive(Debug)]
pub struct BufferMut(bytes::BytesMut);

impl Deref for BufferMut {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct BufferUninit(bytes::BytesMut);

impl BufferUninit {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(bytes::BytesMut::with_capacity(capacity))
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    pub unsafe fn assume_init(self, len: usize) -> BufferMut {
        assert!(len <= self.0.capacity());
        let mut inner = self.0;

        unsafe {
            inner.set_len(len);
        }

        BufferMut(inner)
    }

    pub(crate) fn addr_and_len(&mut self) -> (usize, usize) {
        debug_assert!(self.0.len() == 0);
        let spare = self.0.spare_capacity_mut();
        (spare.as_ptr() as usize, spare.len())
    }
}
