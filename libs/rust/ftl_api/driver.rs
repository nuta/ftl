pub trait Endianess {
    fn into_host_u16(&self, n: u16) -> u16;
    fn into_host_u32(&self, n: u32) -> u32;
    fn into_host_u64(&self, n: u64) -> u64;
    fn from_host_u16(&self, n: u16) -> u16;
    fn from_host_u32(&self, n: u32) -> u32;
    fn from_host_u64(&self, n: u64) -> u64;
}

pub struct LittleEndian;

impl Endianess for LittleEndian {
    fn into_host_u16(&self, n: u16) -> u16 {
        u16::from_le(n)
    }
    fn into_host_u32(&self, n: u32) -> u32 {
        u32::from_le(n)
    }
    fn into_host_u64(&self, n: u64) -> u64 {
        u64::from_le(n)
    }
    fn from_host_u16(&self, n: u16) -> u16 {
        u16::to_le(n)
    }
    fn from_host_u32(&self, n: u32) -> u32 {
        u32::to_le(n)
    }
    fn from_host_u64(&self, n: u64) -> u64 {
        u64::to_le(n)
    }
}

pub struct MmioReg<Endian, T: Copy> {}

impl<Endian, T: Copy> MmioReg<Endian, T> {
    pub const fn new() -> MmioReg<Endian, T> {
        MmioReg {}
    }
}

static A: MmioReg<LittleEndian, u32> = MmioReg::new();

