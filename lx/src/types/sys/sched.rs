use crate::types::c_ulong;

pub const CLONE_FLAGS_MASK: c_ulong = 0x0000_00ff;
pub const CLONE_VM: c_ulong = 0x0000_0100;
pub const CLONE_FS: c_ulong = 0x0000_0200;
pub const CLONE_FILES: c_ulong = 0x0000_0400;
pub const CLONE_SIGHAND: c_ulong = 0x0000_0800;
pub const CLONE_THREAD: c_ulong = 0x0001_0000;
pub const CLONE_SYSVSEM: c_ulong = 0x0004_0000;
pub const CLONE_SETTLS: c_ulong = 0x0008_0000;
pub const CLONE_PARENT_SETTID: c_ulong = 0x0010_0000;
pub const CLONE_CHILD_CLEARTID: c_ulong = 0x0020_0000;
pub const CLONE_DETACHED: c_ulong = 0x0040_0000;
