use super::SyscallResult;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::sysinfo::SysInfo;

pub fn sys_sysinfo(_current: &Thread, buf: *mut SysInfo) -> Result<SyscallResult, Errno> {
    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    // FIXME: Return actual values.
    let info = SysInfo {
        totalram: 1024 * 1024 * 1024,
        freeram: 1024 * 1024 * 1024,
        procs: 1,
        mem_unit: 1,
        ..Default::default()
    };

    unsafe { buf.write(info) };
    Ok(SyscallResult::Done(0))
}
