/// The return values of syscalls higher than or equal to this value indicate
/// error codes (`ERROR_RETVAL_BASE + error`).
pub const ERROR_RETVAL_BASE: usize = {
    // Assuming ErrorCode is 8 bits wide.
    if cfg!(target_pointer_width = "64") {
        0xffff_ffff_ffff_ff00
    } else {
        0xffff_ff00
    }
};

pub const SYS_CONSOLE_WRITE: usize = 1;
pub const SYS_PCI_LOOKUP: usize = 2;
pub const SYS_PCI_SET_BUSMASTER: usize = 3;
pub const SYS_PCI_GET_BAR: usize = 4;
pub const SYS_DMABUF_ALLOC: usize = 5;
pub const SYS_X64_IOPL: usize = 6;
pub const SYS_CHANNEL_CREATE: usize = 7;
