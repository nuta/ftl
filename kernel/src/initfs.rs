//! InitFS: a initramfs-like file system for booting the operating system.
//!
//! The format is a simple uncompressed tar archive.

use core::slice;
pub struct File {
    pub name: &'static str,
    pub data: &'static [u8],
}

pub struct InitFs {
    pub data: &'static [u8],
}

impl InitFs {
    pub fn new(data: &'static [u8]) -> Self {
        Self { data }
    }

    pub fn iter(&self) -> impl Iterator<Item = File> {
        TarIter {
            tarball: self.data,
            pos: 0,
            eof: false,
        }
    }
}

struct TarIter {
    tarball: &'static [u8],
    pos: usize,
    eof: bool,
}

#[repr(C, packed)]
struct TarHeader {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    mtime: [u8; 12],
    checksum: [u8; 8],
    file_type: [u8; 1],
    linkname: [u8; 100],
    magic: [u8; 6],
    version: [u8; 2],
    user_name: [u8; 32],
    group_name: [u8; 32],
    major_device: [u8; 8],
    minor_device: [u8; 8],
    prefix: [u8; 155],
    padding: [u8; 12],
}

fn cstr2str(cstr: &[u8]) -> &str {
    let mut i = 0;
    while i < cstr.len() && cstr[i] != 0 {
        i += 1;
    }

    // TODO: Avoid having UTF-8 table in the kernel: we only need ASCII.
    core::str::from_utf8(&cstr[..i]).expect("invalid file name in initfs")
}

fn oct2int(oct: &[u8]) -> usize {
    let mut dec = 0;
    for &c in oct {
        if c < b'0' || c > b'7' {
            break;
        }
        dec = dec * 8 + (c - b'0') as usize;
    }
    dec
}

impl Iterator for TarIter {
    type Item = File;

    fn next(&mut self) -> Option<Self::Item> {
        if self.eof {
            return None;
        }

        if self.pos + size_of::<TarHeader>() >= self.tarball.len() {
            self.eof = true;
            return None;
        }

        let header = unsafe { &*(self.tarball.as_ptr().add(self.pos) as *const TarHeader) };
        let name = cstr2str(&header.name);
        if name.is_empty() {
            self.eof = true;
            return None;
        }

        let size = oct2int(&header.size);

        self.pos += size_of::<TarHeader>();
        if self.pos + size > self.tarball.len() {
            self.eof = true;
            return None;
        }

        let data = unsafe { slice::from_raw_parts(self.tarball.as_ptr().add(self.pos), size) };
        self.pos += size;

        Some(File { name, data })
    }
}
