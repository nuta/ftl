use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::sync::atomic::AtomicI32;
use core::sync::atomic::Ordering;

use ftl::syscall::vmo_create;
use ftl::syscall::vmo_write;
use ftl::syscall::vmspace_clone;
use ftl::syscall::vmspace_map;
use ftl_elf::Elf;
use ftl_elf::PF_R;
use ftl_elf::PF_W;
use ftl_elf::PF_X;
use ftl_elf::PhdrType;
use ftl_types::handle::HandleId;
use ftl_types::thread::RegsKind;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;
use ftl_utils::spinlock::SpinLock;

use crate::arch::fork_child_entry;
use crate::thread::Thread;
use crate::types::c_int;
use crate::types::errno::Errno;

const PAGE_SIZE: usize = 4096; // TODO: system call?
const STACK_START: usize = 0x0200_0000;
const STACK_SIZE: usize = 256 * 1024;
static NEXT_PID: AtomicI32 = AtomicI32::new(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PId(c_int);

impl fmt::Display for PId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PId {
    pub fn as_int(self) -> c_int {
        self.0
    }
}

#[derive(Clone, Copy)]
struct Mapping {
    start: usize,
    len: usize,
    attrs: PageAttrs,
}

struct Mutable {
    threads: Vec<Arc<Thread>>,
}

pub struct Process {
    tgid: PId,
    isolate: HandleId,
    root_vmspace: HandleId,
    mappings: Vec<Mapping>,
    mutable: SpinLock<Mutable>,
}

impl Process {
    pub fn new_init(
        root_isolate: HandleId,
        root_vmspace: HandleId,
        elf_file: &[u8],
    ) -> Result<Arc<Process>, Errno> {
        let vmspace = vmspace_clone(root_vmspace)?;
        let mut mappings = Vec::new();
        let entry = load_elf(vmspace, elf_file, &mut mappings);
        let stack = vmo_create(STACK_SIZE)?;
        vmspace_map(
            vmspace,
            stack,
            STACK_START,
            PageAttrs::READ | PageAttrs::WRITE,
        )?;
        mappings.push(Mapping {
            start: STACK_START,
            len: STACK_SIZE,
            attrs: PageAttrs::READ | PageAttrs::WRITE,
        });

        // TODO: implement argc, argv, envp, auxv
        let sp = align_down(STACK_START + STACK_SIZE - 5 * size_of::<usize>(), 16);

        let process = Self::new(
            root_isolate,
            root_vmspace,
            vmspace,
            PId(1),
            mappings,
            entry,
            sp,
            |_thread| Ok(()),
        )?;

        Ok(process)
    }

    fn new<F>(
        isolate: HandleId,
        root_vmspace: HandleId,
        vmspace: HandleId,
        tgid: PId,
        mappings: Vec<Mapping>,
        entry: usize,
        sp: usize,
        thread_prestart: F,
    ) -> Result<Arc<Self>, Errno>
    where
        F: FnOnce(&Arc<Thread>) -> Result<(), Errno>,
    {
        let process = Arc::new(Self {
            tgid,
            isolate,
            root_vmspace,
            mappings,
            mutable: SpinLock::new(Mutable {
                threads: Vec::with_capacity(1),
            }),
        });

        // TODO: LX assumes that the cookie won't be dereferenced until the
        // thread is started. Should we document and guarantee this?
        let thread = Thread::new(isolate, vmspace, entry, sp, Arc::downgrade(&process), tgid)?;
        thread_prestart(&thread)?;

        // Start the thread.
        process.mutable.lock().threads.push(thread.clone());
        thread.start()?;

        Ok(process)
    }

    pub fn fork(self: &Arc<Self>, current: &Thread, syscall_sp: usize) -> Result<PId, Errno> {
        let vmspace = vmspace_clone(self.root_vmspace)?;

        // Copy memory into the child's VM space.
        // TODO: copy on write
        for mapping in &self.mappings {
            let vmo = vmo_create(mapping.len)?;
            let bytes =
                unsafe { core::slice::from_raw_parts(mapping.start as *const u8, mapping.len) };
            vmo_write(vmo, 0, bytes)?;
            vmspace_map(vmspace, vmo, mapping.start, mapping.attrs)?;
        }

        // FIXME: PID table to check conflicts
        let tgid = PId(NEXT_PID.fetch_add(1, Ordering::Relaxed));

        // Create a new process and the first thread.
        let entry = fork_child_entry as *const () as usize;
        let child = Self::new(
            self.isolate,
            self.root_vmspace,
            vmspace,
            tgid,
            self.mappings.clone(),
            entry,
            syscall_sp,
            |thread| {
                current.copy_regs_to(&thread, RegsKind::FsBase)?;
                current.copy_regs_to(&thread, RegsKind::FpAndVector)?;
                Ok(())
            },
        )?;

        // FIXME: PID table to keep the ref count
        core::mem::forget(child);

        Ok(tgid)
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

fn load_elf(vmspace: HandleId, elf_file: &[u8], mappings: &mut Vec<Mapping>) -> usize {
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
        mappings.push(Mapping {
            start: region_base,
            len: region_len,
            attrs,
        });
    }

    elf.ehdr.e_entry as usize
}
