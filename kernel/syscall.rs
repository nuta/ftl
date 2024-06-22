use core::mem::offset_of;

use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::message::MessageBuffer;
use ftl_types::message::MessageInfo;
use ftl_types::syscall::SyscallNumber;
use ftl_types::syscall::VsyscallPage;

use crate::channel::Channel;
use crate::cpuvar::current_thread;

pub const VSYSCALL_PAGE: VsyscallPage = VsyscallPage {
    entry: syscall_entry,
};

fn channel_send(handle: HandleId, msginfo: MessageInfo, data: &[u8]) -> Result<(), FtlError> {
    let ch = {
        current_thread()
            .process()
            .handles()
            .lock()
            .get_owned::<Channel>(handle)?
    };

    ch.send(msginfo, data)
}

fn channel_recv(handle: HandleId, data: &mut [u8]) -> Result<MessageInfo, FtlError> {
    let ch = {
        current_thread()
            .process()
            .handles()
            .lock()
            .get_owned::<Channel>(handle)?
    };

    ch.recv()
}

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
            todo!()
        }
        _ if n == SyscallNumber::ChannelSend as isize => {
            let handle = HandleId::from_raw(a0 as i32 /* FIXME: */);
            let msginfo = MessageInfo::from_raw(a1);
            let data_addr = (a2 as usize) + offset_of!(MessageBuffer, data);
            let buf =
                unsafe { core::slice::from_raw_parts(data_addr as *const u8, msginfo.data_len()) };
            channel_send(handle, msginfo, buf)?;
            Ok(0)
        }
        _ if n == SyscallNumber::ChannelRecv as isize => {
            let handle = HandleId::from_raw(a0 as i32 /* FIXME: */);
            let data_addr = (a2 as usize) + offset_of!(MessageBuffer, data);
            let buf = unsafe {
                core::slice::from_raw_parts_mut(data_addr as *mut u8, 0 /* FIXME: */)
            };

            let msginfo = channel_recv(handle, buf)?;
            Ok(msginfo.as_raw())
        }
        _ => {
            println!(
                "unknown syscall: n={}, a0={}, a1={}, a2={}, a3={}, a4={}, a5={}",
                n, a0, a1, a2, a3, a4, a5
            );

            Err(FtlError::UnknownSyscall)
        }
    }
}
