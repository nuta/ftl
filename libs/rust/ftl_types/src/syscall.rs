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
pub const SYS_CHANNEL_SEND: usize = 8;
pub const SYS_SINK_ADD: usize = 9;
pub const SYS_SINK_WAIT: usize = 10;
pub const SYS_SINK_CREATE: usize = 11;
pub const SYS_CHANNEL_OOL_READ: usize = 12;
pub const SYS_CHANNEL_OOL_WRITE: usize = 13;
pub const SYS_INTERRUPT_ACQUIRE: usize = 14;
pub const SYS_INTERRUPT_ACKNOWLEDGE: usize = 15;
pub const SYS_PCI_GET_INTERRUPT_LINE: usize = 16;
pub const SYS_SINK_REMOVE: usize = 17;
pub const SYS_HANDLE_CLOSE: usize = 18;
pub const SYS_PROCESS_EXIT: usize = 19;
pub const SYS_TIME_NOW: usize = 20;
pub const SYS_TIMER_CREATE: usize = 21;
pub const SYS_TIMER_SET: usize = 22;
pub const SYS_SERVICE_REGISTER: usize = 23;
pub const SYS_SERVICE_LOOKUP: usize = 24;
pub const SYS_PCI_GET_SUBSYSTEM_ID: usize = 25;
