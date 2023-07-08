use std::mem::size_of;

use essentials::static_assert;

// "\xbfBFS"
pub const BOOTFS_MAGIC: u32 = 0x424653bf;

pub struct BootfsHeader {
    pub magic: u32,
    pub num_entries: u32,
}

#[repr(u8)]
pub enum EntryType {
    File = 1,
}

pub const NAME_LEN_MAX: usize =
    64 - size_of::<EntryType>() - 2 * size_of::<u32>();

#[repr(C, packed)]
pub struct BootfsEntry {
    pub size: u32,
    pub offset: u32,
    pub entry_type: EntryType,
    /// Null-terminated.
    pub name: [u8; NAME_LEN_MAX],
}

// static_assert!(size_of::<BootfsHeader>() == 64);
