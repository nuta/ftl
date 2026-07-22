use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;

use ftl_api::error::ErrorCode;
use ftl_api::thread::ContextData;
use ftl_api::thread::ContextKind;
use ftl_api::thread::InitRegs;
use ftl_api::thread::SyscallArgs;
use ftl_api::thread::Sysret;
use ftl_api::thread::Thread;
use ftl_api::vmarea::VmArea;
use ftl_api::vmspace::PageAttrs;
use ftl_api::vmspace::VmSpace;
use ftl_elf::ET_EXEC;
use ftl_elf::Elf;
use ftl_elf::PhdrType;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;
use ftl_utils::spinlock::SpinLock;

use crate::elf::STACK_BASE;
use crate::elf::STACK_SIZE;
use crate::elf::build_initial_stack;
use crate::errno::Errno;
use crate::syscall::SyscallOutput;

// TODO: should we use MIN_PAGE_SIZE in FTL?
pub const PAGE_SIZE: usize = 4096;

struct Mutable {
    threads: Vec<Weak<Thread>>,
}

pub struct Process {
    vmspace: VmSpace,
    mutable: SpinLock<Mutable>,
}

impl Process {
    pub fn vmspace(&self) -> &VmSpace {
        &self.vmspace
    }

    pub fn create(elf_file: &[u8]) -> ftl_api::Result<(Arc<Self>, InitRegs)> {
        let elf = Elf::parse(elf_file, ET_EXEC).map_err(|_| ErrorCode::INVALID_ARG)?;
        let vmspace = VmSpace::create()?;

        // Copy the ELF segments into vmareas and map them.
        let e_phoff = elf.ehdr.e_phoff;
        let mut phdr_uaddr = None;
        for phdr in elf.phdrs {
            if phdr.p_type != PhdrType::Load as u32 {
                continue;
            }

            // If the segment contains the program header table, store its uaddr
            // for AT_PHDR.
            if phdr_uaddr.is_none()
                && phdr.p_offset <= e_phoff
                && e_phoff < phdr.p_offset + phdr.p_filesz
            {
                let offset_in_segment = e_phoff - phdr.p_offset;
                phdr_uaddr = Some(phdr.p_vaddr + offset_in_segment);
            }

            let vaddr = phdr.p_vaddr as usize;
            let mapped_vaddr = align_down(vaddr, PAGE_SIZE);
            let vaddr_offset = vaddr - mapped_vaddr;
            let len = align_up(vaddr_offset + phdr.p_memsz as usize, PAGE_SIZE);

            let vmarea = VmArea::allocate(len)?;

            let file_off = phdr.p_offset as usize;
            let file_size = phdr.p_filesz as usize;
            vmarea.write(vaddr_offset, &elf_file[file_off..file_off + file_size])?;

            let mut attrs = PageAttrs::READ;
            if phdr.writable() {
                attrs = attrs | PageAttrs::WRITE;
            }
            if phdr.executable() {
                attrs = attrs | PageAttrs::EXEC;
            }

            vmspace.map(&vmarea, mapped_vaddr, attrs)?;
        }

        // https://xkcd.com/221/
        // FIXME: Use a real random value.
        let random = b"4444444444444444";
        let argv: &[&[u8]] = &[b"hello\0"];
        let env: &[&[u8]] = &[];

        // Prepare the initial stack.
        let stack = VmArea::allocate(STACK_SIZE)?;
        let sp = build_initial_stack(&stack, &elf, argv, env, phdr_uaddr, random)
            .expect("TODO: proper error handling");
        vmspace.map(&stack, STACK_BASE, PageAttrs::READ | PageAttrs::WRITE)?;

        let process = Arc::new(Process {
            vmspace,
            mutable: SpinLock::new(Mutable {
                threads: Vec::new(),
            }),
        });

        let init_regs = InitRegs {
            pc: elf.ehdr.e_entry,
            sp: sp as u64,
        };

        Ok((process, init_regs))
    }

    pub fn start(self: &Arc<Self>, init_regs: InitRegs) -> ftl_api::Result<()> {
        let thread = ThreadContext::spawn(self.clone(), init_regs)?;

        let mut mutable = self.mutable.lock();
        mutable.threads.push(Arc::downgrade(&thread));

        Ok(())
    }
}

struct ThreadContext {
    process: Arc<Process>,
}

impl ThreadContext {
    pub fn spawn(process: Arc<Process>, init_regs: InitRegs) -> ftl_api::Result<Arc<Thread>> {
        let thread = Thread::create(
            process.vmspace(),
            Self {
                process: process.clone(),
            },
        )?;
        thread.set_context(ContextKind::InitRegs, &ContextData { init_regs })?;
        thread.unblock()?;
        Ok(thread)
    }
}

impl ftl_api::thread::Handler for ThreadContext {
    fn syscall(&self, thread: &Thread) {
        // Read syscall arguments.
        let mut regs = ContextData {
            syscall_args: SyscallArgs::zeroed(),
        };
        thread
            .get_context(ContextKind::SyscallArgs, &mut regs)
            .expect("get_context failed");
        let args = unsafe { regs.syscall_args };

        match crate::syscall::handle_syscall(&self.process, thread, args) {
            SyscallOutput::Exit => {}
            SyscallOutput::Done(result) => {
                // Set the return value.
                let retval = result.unwrap_or_else(Errno::to_retval);
                thread
                    .set_context(
                        ContextKind::Sysret,
                        &ContextData {
                            sysret: Sysret {
                                retval: retval as u64,
                            },
                        },
                    )
                    .expect("set_context failed");
                thread.unblock().expect("unblock failed");
            }
        }
    }

    fn terminated(&self, _thread: &Thread) {}
}
