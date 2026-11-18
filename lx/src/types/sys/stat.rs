pub const S_IFCHR: u32 = 0o020000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFREG: u32 = 0o100000;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub __pad0: i32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_atime_nsec: i64,
    pub st_mtime: i64,
    pub st_mtime_nsec: i64,
    pub st_ctime: i64,
    pub st_ctime_nsec: i64,
    pub __unused: [i64; 3],
}

impl Stat {
    pub fn new(ino: u64, mode: u32, size: usize) -> Self {
        Self {
            st_ino: ino,
            st_nlink: 1,
            st_mode: mode,
            st_size: size as i64,
            st_blksize: 4096,
            st_blocks: size.div_ceil(512) as i64,
            ..Default::default()
        }
    }
}
