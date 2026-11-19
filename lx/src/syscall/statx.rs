use super::SyscallResult;
use super::newfstatat::sys_newfstatat;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::AT_STATX_SYNC_TYPE;
use crate::types::sys::stat::STATX_BASIC_STATS;
use crate::types::sys::stat::Stat;
use crate::types::sys::stat::Statx;
use crate::types::sys::stat::StatxTimestamp;

fn dev_major(dev: u64) -> u32 {
    (((dev >> 32) & 0xffff_f000) | ((dev >> 8) & 0xfff)) as u32
}

fn dev_minor(dev: u64) -> u32 {
    (((dev >> 12) & 0xffff_ff00) | (dev & 0xff)) as u32
}

pub fn sys_statx(
    current: &LxThread,
    dirfd: c_int,
    path: *const u8,
    flags: c_int,
    buf: *mut Statx,
) -> Result<SyscallResult, Errno> {
    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    let mut stat = Stat::default();
    sys_newfstatat(current, dirfd, path, &mut stat, flags & !AT_STATX_SYNC_TYPE)?;

    let statx = Statx {
        stx_mask: STATX_BASIC_STATS,
        stx_blksize: stat.st_blksize as u32,
        stx_nlink: stat.st_nlink as u32,
        stx_uid: stat.st_uid,
        stx_gid: stat.st_gid,
        stx_mode: stat.st_mode as u16,
        stx_ino: stat.st_ino,
        stx_size: stat.st_size as u64,
        stx_blocks: stat.st_blocks as u64,
        stx_atime: StatxTimestamp::new(stat.st_atime, stat.st_atime_nsec),
        stx_ctime: StatxTimestamp::new(stat.st_ctime, stat.st_ctime_nsec),
        stx_mtime: StatxTimestamp::new(stat.st_mtime, stat.st_mtime_nsec),
        stx_rdev_major: dev_major(stat.st_rdev),
        stx_rdev_minor: dev_minor(stat.st_rdev),
        stx_dev_major: dev_major(stat.st_dev),
        stx_dev_minor: dev_minor(stat.st_dev),
        ..Default::default()
    };

    unsafe { buf.write(statx) };
    Ok(SyscallResult::Done(0))
}
