use super::SyscallResult;
use crate::thread::Thread;
use crate::types::errno::Errno;
use crate::types::sys::utsname::UTSNAME_LEN;
use crate::types::sys::utsname::UtsName;

#[cfg(target_arch = "x86_64")]
const MACHINE: &[u8] = b"x86_64";
#[cfg(target_arch = "aarch64")]
const MACHINE: &[u8] = b"aarch64";

/// Constructs a `struct utsname` field.
const fn field(value: &[u8]) -> [u8; UTSNAME_LEN] {
    let mut buf = [0; UTSNAME_LEN];

    let (dst, _) = buf.split_at_mut(value.len());
    dst.copy_from_slice(value);

    buf
}

pub fn sys_uname(_current: &Thread, buf: *mut UtsName) -> Result<SyscallResult, Errno> {
    if buf.is_null() {
        return Err(Errno::EFAULT);
    }

    let uts = UtsName {
        sysname: field(b"Linux"),
        nodename: field(b"localhost"),
        // We might want to change this version for applications that use the
        // release field to identify features that they can use.
        release: field(b"5.10.274-ftl"),
        version: field(b"#1"),
        machine: field(MACHINE),
        domainname: field(b"(none)"),
    };

    unsafe { buf.write(uts) };
    Ok(SyscallResult::Done(0))
}
