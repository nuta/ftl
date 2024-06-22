use ftl_types::error::FtlError;
use ftl_types::syscall::SyscallNumber;
use ftl_types::syscall::VsyscallPage;

pub const VSYSCALL_PAGE: VsyscallPage = VsyscallPage {
    entry: syscall_entry,
};

pub fn syscall_entry(
    n: isize,
    a0: isize,
    a1: isize,
    a2: isize,
    a3: isize,
    a4: isize,
    a5: isize,
) -> Result<isize, FtlError> {
    match n {
        _ if n == SyscallNumber::Print as isize => {
            let bytes = unsafe { core::slice::from_raw_parts(a0 as *const u8, a1 as usize) };
            let s = core::str::from_utf8(bytes).unwrap().trim_end();
            println!("[print] {}", s);
            Ok(0)
        }
        _ if n == SyscallNumber::ChannelCreate as isize => {
            let (handle0, handle1) = crate::channel::channel_create()?;
            Ok(handle0 as isize)
        }
        _ if n == SyscallNumber::ChannelSend as isize => {
            let handle = a0 as usize;
            let header = a1 as usize;
            let buf = unsafe { core::slice::from_raw_parts(a2 as *const u8, a3 as usize) };
            crate::channel::channel_send(handle, header, buf)
        }
        _ => {
            println!(
                "unknown syscall: n={}, a0={}, a1={}, a2={}, a3={}, a4={}",
                n, a0, a1, a2, a3, a4
            );

            Err(FtlError::UnknownSyscall)
        }
    }
}
