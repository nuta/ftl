use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::CStr;

use ftl_types::thread::ExitReason;

use super::SyscallResult;
use crate::initfs::InitFsLoader;
use crate::thread::LxThread;
use crate::types::errno::Errno;
use crate::vfs::EmbeddedFile;

fn read_strings<'a>(array: *const *const u8) -> Vec<&'a [u8]> {
    let mut strings = Vec::new();
    if !array.is_null() {
        for i in 0.. {
            let ptr = unsafe { *array.add(i) };
            if ptr.is_null() {
                break;
            }

            let s = unsafe { CStr::from_ptr(ptr.cast()) };
            strings.push(s.to_bytes());
        }
    }

    strings
}

pub fn sys_execve(
    current: &LxThread,
    path: *const u8,
    argv: *const *const u8,
    envp: *const *const u8,
) -> Result<SyscallResult, Errno> {
    let argv_vec = read_strings(argv);
    let envp_vec = read_strings(envp);

    if argv_vec.is_empty() {
        return Err(Errno::EINVAL);
    }

    if path.is_null() {
        return Err(Errno::EFAULT);
    }
    let path = unsafe { CStr::from_ptr(path.cast()) }.to_bytes();

    // TODO: VFS support
    let mut initfs = InitFsLoader::new(&crate::INITFS.0);
    let initfs_file = initfs
        .find(|file| file.name == path.trim_prefix(b"/"))
        .ok_or(Errno::ENOENT)?;
    let elf_file = Arc::new(EmbeddedFile::new(initfs_file.data));

    current
        .process()
        .exec(current, elf_file, &argv_vec, &envp_vec)?;
    Ok(SyscallResult::Exit(ExitReason::Success))
}
