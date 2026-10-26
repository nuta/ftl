use ftl_elf::Elf;
use ftl_elf::PhdrType;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;
use ftl_utils::alignment::is_aligned;

use crate::address::UAddr;
use crate::arch::MIN_PAGE_SIZE;
use crate::arch::USER_ADDR_END;
use crate::boot::BootInfo;
use crate::console::Console;
use crate::handle::Handle;
use crate::hspace::HandleSpace;
use crate::nic::GLOBAL_NIC;
use crate::nic::Nic;
use crate::quota::Quota;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;
use crate::vmobject::VmObject;
use crate::vmspace::VmSpace;

fn load_elf(
    quota: &SharedRef<Quota>,
    vmspace: &SharedRef<VmSpace>,
    image: &SharedRef<VmObject>,
    elf: &Elf,
) -> usize {
    // Load the segments into the allocated memory.
    for phdr in elf.phdrs {
        if phdr.p_type != PhdrType::Load as u32 || phdr.p_memsz == 0 {
            continue;
        }

        if phdr.p_vaddr.saturating_add(phdr.p_memsz) as usize >= USER_ADDR_END {
            panic!(
                "ELF segment exceeds user address space: vaddr={}, memsz={}",
                phdr.p_vaddr, phdr.p_memsz
            );
        }

        assert!(phdr.p_filesz <= phdr.p_memsz);
        assert!(is_aligned(phdr.p_vaddr as usize, MIN_PAGE_SIZE));

        let offset = phdr.p_offset as usize;
        let region_len = align_up(phdr.p_memsz as usize, MIN_PAGE_SIZE);
        let attrs = PageAttrs::from_phdr_flags(phdr.p_flags);
        let uaddr = UAddr::new(phdr.p_vaddr as usize);
        if attrs.contains(PageAttrs::WRITE) {
            // This segment is writeable. Create a new VMO.
            let vmo = image.snapshot(quota.clone(), offset, region_len).unwrap();
            let vmo = Handle::new(vmo, HandleRight::READ | HandleRight::WRITE);
            vmspace.map(vmo, uaddr, 0, region_len, attrs).unwrap();
        } else {
            // This segment is read-only. Map the image VMO directly.
            let vmo = Handle::new(image.clone(), HandleRight::READ);
            vmspace.map(vmo, uaddr, offset, region_len, attrs).unwrap();
        }
    }

    elf.ehdr.e_entry as usize
}

/// Creates a VMO containing `elf_file`.
fn create_elf_vmo(quota: &SharedRef<Quota>, elf: &Elf, elf_file: &[u8]) -> SharedRef<VmObject> {
    let mut file_len = 0;
    let mut image_len = 0;
    for phdr in elf.phdrs {
        if phdr.p_type != PhdrType::Load as u32 {
            continue;
        }

        let offset = phdr.p_offset as usize;

        // -z separate-loadable-segments should ensure these assertions hold.
        assert!(is_aligned(offset, MIN_PAGE_SIZE)); // file offset must be page aligned
        assert!(offset >= image_len); // ELF segments must not overlap in the image

        // Skip .bss segments.
        if phdr.p_filesz > 0 {
            file_len = offset + phdr.p_filesz as usize;
        }
        image_len = align_up(offset + phdr.p_memsz as usize, MIN_PAGE_SIZE);
    }

    // FIXME: this is very inefficient
    let vmo = VmObject::new_zeroed(quota.clone(), image_len).unwrap();
    vmo.write(0, &elf_file[..file_len]).unwrap();
    vmo
}

fn write_stack(
    vmo: &SharedRef<VmObject>,
    stack_bottom: UAddr,
    stack_size: usize,
    cmdline: &[u8],
) -> usize {
    assert!(cmdline.len() <= stack_size / 2, "too long cmdline");

    // Calculate the layout of the stack.
    //
    // The stack pointer is aligned to 16 bytes, x86-64 ABI requirement.
    let cmdline_offset = stack_size - cmdline.len();
    let sp_offset = align_down(cmdline_offset - 2 * size_of::<usize>(), 16);
    let cmdline_ptr = stack_bottom.as_usize() + cmdline_offset;

    // Write cmdline to the stack.
    vmo.write(cmdline_offset, cmdline).unwrap();

    // Push cmdline pointer and its length to the stack.
    vmo.write(sp_offset, &cmdline_ptr.to_ne_bytes()).unwrap();
    vmo.write(sp_offset + 8, &cmdline.len().to_ne_bytes())
        .unwrap();

    sp_offset
}

fn prepare_stack(quota: &SharedRef<Quota>, vmspace: &SharedRef<VmSpace>, cmdline: &[u8]) -> usize {
    let stack_size = 256 * 1024;
    let vmo = VmObject::new_zeroed(quota.clone(), stack_size).unwrap();

    let stack_bottom = UAddr::new(0x40000000 - stack_size); // TODO: find an empty region in vmspace
    let sp_offset = write_stack(&vmo, stack_bottom, stack_size, cmdline);

    let vmo = Handle::new(vmo, HandleRight::READ | HandleRight::WRITE);
    vmspace
        .map(
            vmo,
            stack_bottom,
            0,
            stack_size,
            PageAttrs::READ | PageAttrs::WRITE,
        )
        .unwrap();
    stack_bottom.as_usize() + sp_offset
}

pub fn load(bootinfo: &BootInfo) {
    let initrd = bootinfo.modules.get(0).expect("initrd not found");
    let elf_file = initrd.as_bytes();
    let quota = Quota::new_root().unwrap();
    let vmspace = VmSpace::new(quota.clone())
        .and_then(SharedRef::new)
        .unwrap();
    let elf = Elf::parse(elf_file, ftl_elf::ET_EXEC).expect("failed to parse ELF file");
    let image = create_elf_vmo(&quota, &elf, elf_file);
    let entry = load_elf(&quota, &vmspace, &image, &elf);
    let sp = prepare_stack(&quota, &vmspace, bootinfo.cmdline);

    let hspace = SharedRef::new(HandleSpace::new()).unwrap();
    let vmspace_handle = Handle::new(vmspace.clone(), HandleRight::READ | HandleRight::WRITE);
    let quota_handle = Handle::new(quota.clone(), HandleRight::CHARGE);
    let image_handle = Handle::new(image, HandleRight::READ);
    let hspace_handle = Handle::new(hspace.clone(), HandleRight::WRITE);
    let console = SharedRef::new(Console::new()).unwrap();
    let console_handle = Handle::new(console, HandleRight::READ | HandleRight::WRITE);
    hspace.insert_at(HandleId::new(1), vmspace_handle).unwrap();
    hspace.insert_at(HandleId::new(2), quota_handle).unwrap();
    hspace.insert_at(HandleId::new(3), image_handle).unwrap();
    hspace.insert_at(HandleId::new(4), hspace_handle).unwrap();
    hspace.insert_at(HandleId::new(5), console_handle).unwrap();
    let pnic = GLOBAL_NIC.lock().clone();
    if let Some(pnic) = pnic {
        let pnic = SharedRef::new(Nic::new(pnic)).unwrap();
        let pnic_handle = Handle::new(pnic, HandleRight::READ | HandleRight::WRITE);
        hspace.insert_at(HandleId::new(6), pnic_handle).unwrap();
    }
    let thread = Thread::new(&quota, hspace, vmspace, 0, 0, 0).unwrap();
    thread.start(entry, sp).unwrap();
}
