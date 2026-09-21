use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::slice;

use ftl::vmspace::VmSpace;
use ftl_elf::PF_R;
use ftl_elf::PF_W;
use ftl_elf::PF_X;
use ftl_types::vmspace::PageAttrs;

use crate::types::errno::Errno;
use crate::vfs::FileLike;

fn attrs_from_phdr(phdr: &ftl_elf::Phdr) -> PageAttrs {
    let mut attrs = PageAttrs::EMPTY;
    if phdr.p_flags & PF_X != 0 {
        attrs |= PageAttrs::EXEC;
    }

    if phdr.p_flags & PF_W != 0 {
        attrs |= PageAttrs::WRITE;
    }

    if phdr.p_flags & PF_R != 0 {
        attrs |= PageAttrs::READ;
    }

    attrs
}

fn read_exact(file: &dyn FileLike, mut offset: usize, buf: &mut [u8]) -> Result<(), Errno> {
    let mut total = 0;
    while total < buf.len() {
        let n = file.read(&mut buf[total..], offset, false)?;
        assert!(n > 0); // FIXME: proper errno
        total += n;
        offset += n;
    }

    Ok(())
}

fn read_uninit<T: Copy>(
    file: &dyn FileLike,
    offset: usize,
    buf: &mut MaybeUninit<T>,
) -> Result<T, Errno> {
    let slice = unsafe { slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, size_of::<T>()) };
    read_exact(file, offset, slice)?;
    // SAFETY: read_exact guarantees that the buffer is filled.
    Ok(unsafe { buf.assume_init() })
}

struct LoadedElf {
    entry: usize,
    phdr: usize,
    phent: usize,
    phnum: usize,
}

fn load_elf(
    vmspace: &VmSpace,
    elf_file: &dyn FileLike,
    mappings: &mut Vec<Mapping>,
) -> Result<LoadedElf, Errno> {
    let mut ehdr = MaybeUninit::<ftl_elf::Ehdr>::uninit();
    let ehdr = read_uninit(elf_file, 0, &mut ehdr)?;

    let phdrs_end = ehdr.e_phoff as usize + ehdr.e_phnum as usize * size_of::<ftl_elf::Phdr>();
    let mut header_region = vec![0u8; phdrs_end]; // TODO: Use MaybeUninit
    read_exact(elf_file, 0, &mut header_region)?;

    let elf = Elf::parse(&header_region, ftl_elf::ET_EXEC).expect("failed to parse ELF");
    let mut phdr_vaddr = 0;
    for phdr in elf.phdrs {
        if phdr.p_type == PhdrType::Phdr as u32 {
            phdr_vaddr = phdr.p_vaddr as usize;
        }

        if phdr.p_type != PhdrType::Load as u32 {
            continue;
        }

        let vaddr = phdr.p_vaddr as usize;
        let region_base = align_down(vaddr, PAGE_SIZE);
        let page_offset = vaddr - region_base;
        let region_len = align_up(page_offset + phdr.p_memsz as usize, PAGE_SIZE);
        let vmo = Vmo::create(region_len).unwrap();

        let filesz = phdr.p_filesz as usize;
        let mut buf = [0u8; PAGE_SIZE];
        let mut offset = 0;
        while offset < filesz {
            let len = min(buf.len(), filesz - offset);
            let chunk = &mut buf[..len];
            // FIXME: do not copy twice
            read_exact(elf_file, phdr.p_offset as usize + offset, chunk)?;
            vmo.write(page_offset + offset, chunk)?;
            offset += len;
        }

        let attrs = attrs_from_phdr(phdr);
        vmspace.map(&vmo, region_base, attrs)?;
        mappings.push(Mapping {
            start: region_base,
            len: region_len,
            attrs,
        });
    }

    Ok(LoadedElf {
        entry: elf.ehdr.e_entry as usize,
        phdr: phdr_vaddr,
        phent: elf.ehdr.e_phentsize as usize,
        phnum: elf.ehdr.e_phnum as usize,
    })
}
