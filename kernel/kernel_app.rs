use core::mem::size_of;

use ftl_elf::Elf;
use ftl_elf::PhdrType;
use ftl_elf::Rela;
use ftl_types::syscall::VsyscallPage;
use ftl_utils::alignment::align_up;

use crate::arch::PAGE_SIZE;
use crate::handle::AnyHandle;
use crate::handle::HandleRights;
use crate::handle::Handleable;
use crate::memory::AllocPagesError;
use crate::memory::AllocatedPages;
use crate::process::Process;
use crate::ref_counted::SharedRef;
use crate::thread::Thread;

#[derive(Debug)]
pub enum Error {
    ParseElf(ftl_elf::ParseError),
    NoPhdrs,
    AllocPages(AllocPagesError),
}

pub struct KernelAppMemory {
    #[allow(dead_code)]
    pages: AllocatedPages,
}

impl Handleable for KernelAppMemory {}

pub struct KernelAppLoader<'a> {
    elf_file: &'a [u8],
    elf: Elf<'a>,
    memory: AllocatedPages,
}

impl<'a> KernelAppLoader<'a> {
    pub fn new(elf_file: &[u8]) -> Result<KernelAppLoader, Error> {
        let elf = Elf::parse(elf_file).map_err(Error::ParseElf)?;

        let lowest_addr = elf
            .phdrs
            .iter()
            .filter(|phdr| phdr.p_type == PhdrType::Load)
            .map(|phdr| phdr.p_vaddr as usize)
            .min()
            .ok_or(Error::NoPhdrs)?;
        let highest_addr = elf
            .phdrs
            .iter()
            .filter(|phdr| phdr.p_type == PhdrType::Load)
            .map(|phdr| (phdr.p_vaddr + phdr.p_memsz) as usize)
            .max()
            .ok_or(Error::NoPhdrs)?;

        let elf_len = align_up(highest_addr - lowest_addr, PAGE_SIZE);
        let memory = AllocatedPages::alloc(elf_len).map_err(Error::AllocPages)?;
        Ok(KernelAppLoader {
            elf_file,
            elf,
            memory,
        })
    }

    fn base_addr(&self) -> usize {
        self.memory.as_ptr() as usize
    }

    fn entry_addr(&self) -> usize {
        self.base_addr() + (self.elf.ehdr.e_entry as usize)
    }

    fn load_segments(&mut self) {
        let memory = self.memory.as_slice_mut();
        for phdr in self.elf.phdrs {
            if phdr.p_type != ftl_elf::PhdrType::Load {
                continue;
            }
            let mem_offset = phdr.p_vaddr as usize;
            let file_offset = phdr.p_offset as usize;
            let file_copy_len = phdr.p_filesz as usize;
            memory[mem_offset..mem_offset + file_copy_len]
                .copy_from_slice(&self.elf_file[file_offset..file_offset + file_copy_len]);
            let zeroed_len = phdr.p_memsz as usize - phdr.p_filesz as usize;
            memory[mem_offset + file_copy_len..mem_offset + file_copy_len + zeroed_len].fill(0);
        }
    }

    fn get_shdr_by_name(&self, name: &str) -> Option<&ftl_elf::Shdr> {
        fn get_cstr(buffer: &[u8], offset: usize) -> Option<&str> {
            let mut len = 0;
            while let Some(&ch) = buffer.get(offset + len) {
                if ch == 0 {
                    return core::str::from_utf8(&buffer[offset..offset + len]).ok();
                }
                len += 1;
            }
            None
        }

        let shstrtab_section = self.elf.shdrs.get(self.elf.ehdr.e_shstrndx as usize)?;
        let shstrtab = unsafe {
            core::slice::from_raw_parts(
                self.elf_file
                    .as_ptr()
                    .add(shstrtab_section.sh_offset as usize),
                shstrtab_section.sh_size as usize,
            )
        };

        self.elf.shdrs.iter().find(|shdr| {
            if let Some(sh_name) = get_cstr(shstrtab, shdr.sh_name as usize) {
                sh_name == name
            } else {
                false
            }
        })
    }

    fn relocate_rela_dyn(&mut self) {
        let rela_dyn = self.get_shdr_by_name(".rela.dyn").unwrap();
        let rela_entries = unsafe {
            assert!(
                rela_dyn.sh_size as usize % size_of::<Rela>() == 0,
                "misaligned .rela_dyn size"
            );
            core::slice::from_raw_parts(
                self.elf_file.as_ptr().add(rela_dyn.sh_offset as usize) as *const Rela,
                (rela_dyn.sh_size as usize) / size_of::<Rela>(),
            )
        };

        for rela in rela_entries {
            unsafe {
                let ptr = (self.base_addr() + rela.r_offset as usize) as *mut i64;
                *ptr += (self.base_addr() as i64) + rela.r_addend;
            };
        }
    }

    pub fn load(mut self, vsyscall_page: *const VsyscallPage) {
        self.load_segments();
        self.relocate_rela_dyn();

        let entry = unsafe { core::mem::transmute(self.entry_addr()) };
        let thread = Thread::spawn_kernel(entry, vsyscall_page as usize);
        let mut proc = Process::create();

        let kernel_app_memory = SharedRef::new(KernelAppMemory { pages: self.memory });

        proc.add_handle(AnyHandle::new(kernel_app_memory, HandleRights(0))).unwrap();
        proc.add_handle(AnyHandle::new(thread, HandleRights(0))).unwrap();

        let _proc = SharedRef::new(proc);
    }
}