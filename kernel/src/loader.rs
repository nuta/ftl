use ftl_elf::Elf;
use ftl_elf::PF_R;
use ftl_elf::PF_W;
use ftl_elf::PF_X;
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
use crate::handle::Handle;
use crate::hspace::HandleSpace;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;
use crate::vmobject::VmObject;
use crate::vmspace::VmSpace;

fn load_elf(vmspace: &SharedRef<VmSpace>, elf_file: &[u8]) -> usize {
    let elf = Elf::parse(elf_file, ftl_elf::ET_EXEC).expect("failed to parse ELF file");

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

        // Copy the file contents to the allocated memory.
        let src_off = phdr.p_offset as usize;
        let copy_len = phdr.p_filesz as usize;
        let region_len = align_up(phdr.p_memsz as usize, MIN_PAGE_SIZE);
        let bytes = &elf_file[src_off..src_off + copy_len];

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

        // Copy the file contents to the allocated memory.
        let vmo = VmObject::new_anonymous(region_len).unwrap();
        vmo.write(0, bytes).unwrap();

        // Map the region to the address space.
        vmspace
            .map(vmo, UAddr::new(phdr.p_vaddr as usize), attrs)
            .unwrap();
    }

    elf.ehdr.e_entry as usize
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

fn prepare_stack(vmspace: &SharedRef<VmSpace>, cmdline: &[u8]) -> usize {
    let stack_size = 256 * 1024;
    let vmo = VmObject::new_anonymous(stack_size).unwrap();

    let stack_bottom = UAddr::new(0x40000000 - stack_size); // TODO: find an empty region in vmspace
    let sp_offset = write_stack(&vmo, stack_bottom, stack_size, cmdline);

    vmspace
        .map(vmo, stack_bottom, PageAttrs::READ | PageAttrs::WRITE)
        .unwrap();
    stack_bottom.as_usize() + sp_offset
}

pub fn load(bootinfo: &BootInfo) {
    let initrd = bootinfo.modules.get(0).expect("initrd not found");
    let elf_file = initrd.as_bytes();
    let vmspace = VmSpace::new().and_then(SharedRef::new).unwrap();
    let entry = load_elf(&vmspace, elf_file);
    let sp = prepare_stack(&vmspace, bootinfo.cmdline);

    let hspace = SharedRef::new(HandleSpace::new()).unwrap();
    let hspace_handle = Handle::new(hspace.clone(), HandleRight::WRITE);
    let vmspace_handle = Handle::new(
        vmspace.clone(),
        HandleRight::READ | HandleRight::WRITE | HandleRight::MAP,
    );
    hspace.insert_at(HandleId::new(1), hspace_handle).unwrap();
    hspace.insert_at(HandleId::new(2), vmspace_handle).unwrap();
    let thread = Thread::new(hspace, vmspace, entry, sp, 0, 0).unwrap();
    thread.start().unwrap();
}
