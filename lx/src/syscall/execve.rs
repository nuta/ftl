use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::CStr;

use ftl_types::thread::ExitReason;

use crate::initfs::InitFsLoader;
use crate::thread::LxThread;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::vfs::EmbeddedFile;

pub fn sys_execve(
    current: &LxThread,
    _path: *const u8,
    argv: *const *const u8,
    _envp: *const *const u8,
) -> Result<c_long, Errno> {
    let mut argv_vec = Vec::new();
    if !argv.is_null() {
        for i in 0.. {
            let ptr = unsafe { *argv.add(i) };
            if ptr.is_null() {
                break;
            }

            let arg = unsafe { CStr::from_ptr(ptr.cast()) };
            argv_vec.push(arg.to_bytes_with_nul());
        }
    }

    if argv_vec.is_empty() {
        return Err(Errno::EINVAL);
    }

    // TODO: VFS support
    let mut initfs = InitFsLoader::new(&crate::INITFS.0);
    let initfs_file = initfs
        .find(|file| file.name == argv_vec[0].trim_prefix(b"/"))
        .expect("init not found in initfs");
    let elf_file = Arc::new(EmbeddedFile::new(initfs_file.data));

    // TODO: envp support
    current.process().exec(current, elf_file, &argv_vec)?;
    ftl::thread::exit(ExitReason::Success)
}
