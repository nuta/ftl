use crate::error::FtlError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(isize)]
pub enum SyscallNumber {
    Print = 1,
    ChannelCreate = 2,
    ChannelSend = 3,
    ChannelRecv = 4,
    ChannelCall = 5,
    HandleClose = 6,
}

pub struct VsyscallPage {
    pub entry: fn(isize, isize, isize, isize, isize, isize, isize) -> Result<isize, FtlError>,
}
