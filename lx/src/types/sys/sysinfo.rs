#[derive(Default)]
#[repr(C)]
pub struct SysInfo {
    pub uptime: i64,
    pub loads: [u64; 3],
    pub totalram: u64,
    pub freeram: u64,
    pub sharedram: u64,
    pub bufferram: u64,
    pub totalswap: u64,
    pub freeswap: u64,
    pub procs: u16,
    pub pad: u16,
    pub totalhigh: u64,
    pub freehigh: u64,
    pub mem_unit: u32,
}
