#[derive(Debug)]
pub struct Buffer {
    ptr: *mut u8,
    len: usize,
}

impl Buffer {}

pub struct BufferMut {}

pub(crate) struct BufferCookie {
    usize: usize,
}
