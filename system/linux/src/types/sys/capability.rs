pub const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;

#[repr(C)]
pub struct CapUserHeader {
    pub version: u32,
    pub pid: i32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct CapUserData {
    pub effective: u32,
    pub permitted: u32,
    pub inheritable: u32,
}
