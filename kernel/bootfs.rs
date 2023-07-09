use core::{mem::size_of, str::from_utf8_unchecked};

use bootfs::{BootfsEntry, BootfsHeader};

/// A workaround for the lack of alignment attribute on `include_bytes!`.
#[repr(align(4096))]
struct PageAligned<T: ?Sized>(T);

const BOOTFS_IMAGE: &'static PageAligned<[u8]> =
    &PageAligned(*include_bytes!("../bootfs.bin"));

pub struct Bootfs {
    header: &'static BootfsHeader,
    entries: &'static [BootfsEntry],
}

impl Bootfs {
    pub fn load() -> Bootfs {
        let image = unsafe { BOOTFS_IMAGE.0.as_ptr() };

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
            let len = entry.name.iter().position(|&c| c == b'\0').unwrap();
            let name = unsafe { from_utf8_unchecked(&entry.name[..len]) };
            println!("bootfs: found entry \"{}\"", name);
        }

        Bootfs { header, entries }
    }
}
