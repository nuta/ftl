use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::CStr;

use ftl_types::thread::ExitReason;

use crate::HELLO_ELF;
use crate::thread::Thread;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::vfs::EmbeddedFile;

pub fn sys_execve(
    current: &Thread,
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

    // TODO: load from the path
    let elf_file = Arc::new(EmbeddedFile::new(&HELLO_ELF.0));

    // TODO: envp support
    current.process().exec(current, elf_file, &argv_vec)?;
    ftl::syscall::thread_exit(ExitReason::Success)
}
