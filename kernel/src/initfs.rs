use core::mem::offset_of;
use core::slice;

use ftl_utils::alignment::align_up;

use crate::arch;
use crate::boot::Module;

/// New CPIO header format.
///
/// <https://man.archlinux.org/man/cpio.5.en#New_ASCII_Format>
#[repr(C, packed)]
struct CpioHeader {
    magic: [u8; 6],
    inode: [u8; 8],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    nlink: [u8; 8],
    mtime: [u8; 8],
    filesize: [u8; 8],
    dev_major: [u8; 8],
    dev_minor: [u8; 8],
    rdev_major: [u8; 8],
    rdev_minor: [u8; 8],
    namesize: [u8; 8],
    checksum: [u8; 8],
}

fn parse_hex(s: &[u8]) -> Option<usize> {
    let mut value = 0;
    for &byte in s {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return None,
        };

        value = value * 16 + (digit as usize);
    }

    Some(value)
}

pub struct File<'a> {
    pub name: &'a [u8],
    pub data: &'a [u8],
}

pub struct InitFsLoader<'a> {
    file: &'a [u8],
    offset: usize,
}

impl<'a> InitFsLoader<'a> {
    pub fn new(module: &Module) -> Self {
        let start: *const u8 = arch::paddr2vaddr(module.start).as_ptr();
        let end: *const u8 = arch::paddr2vaddr(module.end).as_ptr();
        let len = (end as usize).saturating_sub(start as usize);
        let file = unsafe { slice::from_raw_parts(start, len) };
        Self { file, offset: 0 }
    }
}

impl<'a> Iterator for InitFsLoader<'a> {
    type Item = File<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let magic = self.file.get(self.offset..self.offset + 6)?;
        if magic != b"070701" {
            return None;
        }

        let namesize_offset = self.offset + offset_of!(CpioHeader, namesize);
        let namesize_bytes = self.file.get(namesize_offset..namesize_offset + 8)?;
        let namesize = parse_hex(namesize_bytes)?;

        let filesize_offset = self.offset + offset_of!(CpioHeader, filesize);
        let filesize_bytes = self.file.get(filesize_offset..filesize_offset + 8)?;
        let filesize = parse_hex(filesize_bytes)?;

        let name_offset = self.offset + size_of::<CpioHeader>();
        let mut name = self.file.get(name_offset..name_offset + namesize)?;

        // Remove the trailing null byte.
        if name.ends_with(b"\0") {
            name = &name[..name.len() - 1];
        }

        if name == b"TRAILER!!!" {
            return None;
        }

        let data_offset = align_up(self.offset + size_of::<CpioHeader>() + namesize, 4);
        let data = self.file.get(data_offset..data_offset + filesize)?;

        self.offset = align_up(data_offset + filesize, 4);
        Some(File { name, data })
    }
}
