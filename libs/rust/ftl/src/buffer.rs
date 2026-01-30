use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug)]
pub enum Buffer {
    Static(&'static [u8]),
    String(String),
    Vec(Vec<u8>),
}

impl Buffer {}

pub enum BufferMut {
    String(String),
    Vec(Vec<u8>),
}
