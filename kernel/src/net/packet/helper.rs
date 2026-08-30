pub fn read_array<const N: usize>(buf: &[u8], offset: usize) -> [u8; N] {
    buf[offset..offset + N].try_into().unwrap()
}

pub fn read_u16(buf: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(read_array(buf, offset))
}

pub fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(read_array(buf, offset))
}

pub fn write_array<const N: usize>(buf: &mut [u8], offset: usize, value: [u8; N]) {
    buf[offset..offset + N].copy_from_slice(&value);
}

pub fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    write_array(buf, offset, value.to_be_bytes());
}

pub fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    write_array(buf, offset, value.to_be_bytes());
}
