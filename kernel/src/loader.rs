use ftl_elf::Elf;
use ftl_elf::PF_R;
use ftl_elf::PF_W;
use ftl_elf::PF_X;
use ftl_elf::PhdrType;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_up;

use crate::address::UAddr;
use crate::arch::MIN_PAGE_SIZE;
use crate::boot::BootInfo;
use crate::handle::Handle;
use crate::initfs::File;
use crate::initfs::InitFsLoader;
use crate::isolate::Isolate;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;
use crate::vmobject::VmObject;
use crate::vmspace::VmSpace;

fn load_elf(vmspace: &SharedRef<VmSpace>, elf_file: &[u8]) -> usize {
    let elf = Elf::parse(elf_file, ftl_elf::ET_EXEC).expect("failed to parse ELF file");

    // Load the segments into the allocated memory.
    for phdr in elf.phdrs {
        if phdr.p_type != PhdrType::Load as u32 {
            continue;
        }

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

        let vmo = VmObject::new_anonymous(region_len).unwrap();
        vmo.write(0, bytes).unwrap();
        vmspace
            .map(vmo, UAddr::new(phdr.p_vaddr as usize), attrs)
            .unwrap();
    }

    elf.ehdr.e_entry as usize
}

fn find_file<'a>(bootinfo: &'a BootInfo, name: &[u8]) -> File<'a> {
    for module in &bootinfo.modules {
        let initfs = InitFsLoader::new(module);
        for file in initfs {
            if file.name == name {
                return file;
            }
        }
    }

    panic!("ELF file not found in initfs");
}

fn prepare_stack(vmspace: &SharedRef<VmSpace>) -> usize {
    let stack_size = 256 * 1024;
    let vmo = VmObject::new_anonymous(stack_size).unwrap();

    let start = UAddr::new(0x100000 - stack_size); // TODO: find an empty region in vmspace

    vmspace
        .map(vmo, start, PageAttrs::READ | PageAttrs::WRITE)
        .unwrap();
    start.as_usize() + stack_size
}

pub fn load(bootinfo: &BootInfo) {
    let elf_file = find_file(bootinfo, b"lx.elf");
    let vmspace = VmSpace::new().and_then(SharedRef::new).unwrap();
    let entry = load_elf(&vmspace, elf_file.data);
    let sp = prepare_stack(&vmspace);

    let isolate = SharedRef::new(Isolate::new()).unwrap();
    {
        let mut handles = isolate.handles().lock();
        let isolate_handle = Handle::new(isolate.clone(), HandleRight::WRITE);
        let vmspace_handle = Handle::new(
            vmspace.clone(),
            HandleRight::READ | HandleRight::WRITE | HandleRight::MAP,
        );
        handles.insert_at(HandleId::new(1), isolate_handle).unwrap();
        handles.insert_at(HandleId::new(2), vmspace_handle).unwrap();
    }
    let thread = Thread::new(isolate, vmspace, entry, sp, 0, 0).unwrap();
    thread.start().unwrap();
}
