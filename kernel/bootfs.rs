use core::{mem::size_of, slice, str::from_utf8_unchecked};

use bootfs::{BootfsEntry, BootfsHeader};

/// A workaround for the lack of alignment attribute on `include_bytes!`.
#[repr(align(4096))]
struct PageAligned<T: ?Sized>(T);

const BOOTFS_IMAGE: &'static PageAligned<[u8]> =
    &PageAligned(*include_bytes!("../bootfs.bin"));

/// Converts a null-terminated C string to `&str`.
///
/// # Panics
///
/// Panics if the input is not null-terminated.
///
/// # Safety
///
/// This function assumes that the input is a valid UTF-8 string.
pub unsafe fn cstr2str(cstr: &[u8]) -> &str {
    let len = cstr.iter().position(|&c| c == b'\0').unwrap();
    unsafe { from_utf8_unchecked(&cstr[..len]) }
}

pub struct Bootfs {
    header: &'static BootfsHeader,
    entries: &'static [BootfsEntry],
}

pub struct BootfsFile {
    pub data: &'static [u8],
}

impl Bootfs {
    pub fn load() -> Bootfs {
        let image = BOOTFS_IMAGE.0.as_ptr();

        // Safety: PageAligned guarantees that the data is correctly aligned.
        let header = unsafe { &*(image as *const BootfsHeader) };
        assert_eq!(header.magic, bootfs::BOOTFS_MAGIC);

        let entries = unsafe {
            core::slice::from_raw_parts(
                image.add(size_of::<BootfsHeader>()) as *const BootfsEntry,
                header.num_entries as usize,
            )
        };

        for entry in entries {
            // Safety: We assume the mkbootfs tool correctly generated the image.
            let name = unsafe { cstr2str(&entry.name) };
            println!("bootfs: found entry \"{}\"", name);
        }

        Bootfs { header, entries }
    }

    pub fn find_by_name(&self, name: &str) -> Option<BootfsFile> {
        for entry in self.entries {
            // TODO: Avoid converting to `&str` every time.
            // Safety: We assume the mkbootfs tool correctly generated the image.
            if unsafe { cstr2str(&entry.name) } == name {
                let image = BOOTFS_IMAGE.0.as_ptr();
                // Safety: We assume the mkbootfs tool correctly generated the image.
                let data = unsafe {
                    slice::from_raw_parts(
                        image.add(entry.offset as usize),
                        entry.size as usize,
                    )
                };
                return Some(BootfsFile { data });
            }
        }
        None
    }
}
