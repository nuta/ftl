use core::mem::size_of;

/// Reads a big-endian 16-bit integer from a byte slice.
pub fn read_u16_be(buf: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(buf[offset..offset + size_of::<u16>()].try_into().unwrap())
}

/// Reads a big-endian 32-bit integer from a byte slice.
pub fn read_u32_be(buf: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(buf[offset..offset + size_of::<u32>()].try_into().unwrap())
}
