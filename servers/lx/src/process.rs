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

use crate::errno::Errno;

const PAGE_SIZE: usize = 4096;
const STACK_BASE: usize = 0x0000_7000_0000_0000;
const STACK_SIZE: usize = 64 * 1024;

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
        for phdr in elf.phdrs {
            if phdr.p_type != PhdrType::Load as u32 {
                continue;
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

        // Prepare the initial stack.
        let stack = VmArea::allocate(STACK_SIZE)?;
        vmspace.map(&stack, STACK_BASE, PageAttrs::READ | PageAttrs::WRITE)?;
        let sp = STACK_BASE + STACK_SIZE;

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

        let result = crate::syscall::handle_syscall(&self.process, thread, args);

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

    fn terminated(&self, _thread: &Thread) {}
}
