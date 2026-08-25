use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

use ftl::syscall::vmo_create;
use ftl::syscall::vmo_write;
use ftl::syscall::vmspace_clone;
use ftl::syscall::vmspace_map;
use ftl_elf::Elf;
use ftl_elf::PF_R;
use ftl_elf::PF_W;
use ftl_elf::PF_X;
use ftl_elf::PhdrType;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;
use ftl_utils::spinlock::SpinLock;

use crate::thread::Thread;

const PAGE_SIZE: usize = 4096; // TODO: system call?
const STACK_START: usize = 0x0200_0000;
const STACK_SIZE: usize = 256 * 1024;

#[derive(Debug)]
pub enum CreateError {
    ThreadCreate(crate::thread::SpawnError),
    ThreadStart(ErrorCode),
    VmSpaceClone(ErrorCode),
    VmSpaceMap(ErrorCode),
    VmoCreate(ErrorCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PId(i32);

impl fmt::Display for PId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

struct Mutable {
    threads: Vec<Arc<Thread>>,
}

pub struct Process {
    tgid: PId,
    mutable: SpinLock<Mutable>,
}

impl Process {
    pub fn new_init(
        root_isolate: HandleId,
        root_vmspace: HandleId,
        elf_file: &[u8],
    ) -> Result<Arc<Process>, CreateError> {
        Self::new(root_isolate, root_vmspace, elf_file, PId(1))
    }

    fn new(
        root_isolate: HandleId,
        root_vmspace: HandleId,
        elf_file: &[u8],
        tgid: PId,
    ) -> Result<Arc<Process>, CreateError> {
        let vmspace = vmspace_clone(root_vmspace).map_err(CreateError::VmSpaceClone)?;
        let entry = load_elf(vmspace, elf_file);
        let stack = vmo_create(STACK_SIZE).map_err(CreateError::VmoCreate)?;
        vmspace_map(
            vmspace,
            stack,
            STACK_START,
            PageAttrs::READ | PageAttrs::WRITE,
        )
        .map_err(CreateError::VmSpaceMap)?;

        // TODO: implement argc, argv, envp, auxv
        let sp = align_down(STACK_START + STACK_SIZE - 5 * size_of::<usize>(), 16);

        let threads = Vec::with_capacity(1);
        let process = Arc::new(Process {
            tgid,
            mutable: SpinLock::new(Mutable { threads }),
        });

        // TODO: LX assumes that the cookie won't be dereferenced until the
        //       thread is started. Should we document and guarantee this?
        let lx_thread = Thread::new(
            root_isolate,
            vmspace,
            entry,
            sp,
            Arc::downgrade(&process),
            tgid,
        )
        .map_err(CreateError::ThreadCreate)?;

        process.mutable.lock().threads.push(lx_thread.clone());
        lx_thread.start().map_err(CreateError::ThreadStart)?;

        Ok(process)
    }
}

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

fn load_elf(vmspace: HandleId, elf_file: &[u8]) -> usize {
    let elf = Elf::parse(elf_file, ftl_elf::ET_EXEC).expect("failed to parse hello ELF");
    for phdr in elf.phdrs {
        if phdr.p_type != PhdrType::Load as u32 {
            continue;
        }

        let vaddr = phdr.p_vaddr as usize;
        let region_base = align_down(vaddr, PAGE_SIZE);
        let page_offset = vaddr - region_base;
        let region_len = align_up(page_offset + phdr.p_memsz as usize, PAGE_SIZE);
        let vmo = vmo_create(region_len).unwrap();

        let file_start = phdr.p_offset as usize;
        let file_end = file_start + phdr.p_filesz as usize;
        vmo_write(vmo, page_offset, &elf_file[file_start..file_end]).unwrap();

        let attrs = attrs_from_phdr(phdr);
        vmspace_map(vmspace, vmo, region_base, attrs).unwrap();
    }

    elf.ehdr.e_entry as usize
}
