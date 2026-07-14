use alloc::vec::Vec;

use ftl_api::error::ErrorCode;
use ftl_api::handle::HandleRight;
use ftl_api::start::StartInfo;
use ftl_utils::spinlock::SpinLock;

use crate::address::UAddr;
use crate::arch;
use crate::boot::BootInfo;
use crate::initfs;
use crate::loader::LoadedElf;
use crate::memory::PAGE_ALLOCATOR;
use crate::memory::PageType;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;
use crate::vmarea::VmArea;
use crate::vmspace::VmSpace;

const START_INFO: &StartInfo = &StartInfo {
    malloc: |size| {
        let paddr = PAGE_ALLOCATOR
            .alloc(size, PageType::Dirty)
            .ok_or(ErrorCode::OUT_OF_MEMORY)?;

        let ptr = arch::paddr2vaddr(paddr).as_mut_ptr();
        Ok(ptr)
    },
    print: |bytes| {
        arch::console_write(bytes);
    },
    panic: || {
        panic!("server panicked");
    },
    vmspace_create: || {
        let vmspace = VmSpace::new()?;
        let handle = SharedRef::new(vmspace)?.into_handle();
        Ok(handle)
    },
    vmarea_allocate: |len| {
        let vmarea = VmArea::new_anonymous(len)?;
        let handle = vmarea.into_handle();
        Ok(handle)
    },
    vmarea_write: |vmarea, offset, data| {
        let vmarea = SharedRef::<VmArea>::from_borrowed_handle(vmarea, HandleRight::WRITE)?;
        vmarea.write(offset, data)
    },
    vmspace_map: |vmspace, vmarea, uaddr, attrs| {
        let vmspace = SharedRef::<VmSpace>::from_borrowed_handle(vmspace, HandleRight::MAP)?;
        let vmarea = SharedRef::<VmArea>::from_borrowed_handle(vmarea, HandleRight::MAP)?;
        vmspace.map(vmarea, UAddr::new(uaddr), attrs)
    },
    vmspace_read: |vmspace, uaddr, buf| {
        let vmspace = SharedRef::<VmSpace>::from_borrowed_handle(vmspace, HandleRight::READ)?;
        vmspace.read_bytes(UAddr::new(uaddr), buf)
    },
    thread_create: |vmspace, upcall| {
        let vmspace = SharedRef::<VmSpace>::from_borrowed_handle(vmspace, HandleRight::MAP)?;
        let thread = Thread::new(vmspace, upcall)?;
        Ok(thread.into_handle())
    },
    thread_get_context: |thread, kind, regs| {
        let thread = SharedRef::<Thread>::from_borrowed_handle(thread, HandleRight::READ)?;
        thread.read_context(kind, regs)
    },
    thread_set_context: |thread, kind, regs| {
        let thread = SharedRef::<Thread>::from_borrowed_handle(thread, HandleRight::WRITE)?;
        thread.write_context(kind, regs)
    },
    thread_unblock: |thread| {
        let thread = SharedRef::<Thread>::from_borrowed_handle(thread, HandleRight::WRITE)?;
        thread.unblock()
    },
    thread_terminate: |thread| {
        let thread = SharedRef::<Thread>::from_borrowed_handle(thread, HandleRight::WRITE)?;
        thread.terminate()
    },
    thread_destroy: |thread| {
        let sref = SharedRef::<Thread>::from_moved_handle(thread)?;
        // Decrement the ref count.
        drop(sref);
        Ok(())
    },
};

static SERVERS: SpinLock<Vec<Server>> = SpinLock::new(Vec::new());

pub struct Server {
    image: *const u8,
}

impl Server {
    fn load(elf_file: &[u8]) -> Result<Self, crate::loader::Error> {
        let LoadedElf { image, entry_fn } = crate::loader::load_elf(elf_file)?;
        entry_fn(START_INFO);
        Ok(Self { image })
    }
}

unsafe impl Send for Server {}

pub fn init(bootinfo: &BootInfo) {
    for module in &bootinfo.modules {
        let initfs = initfs::InitFsLoader::new(module);
        for file in initfs {
            if file.name.starts_with(b"servers/") && file.name.ends_with(b".elf") {
                let name = core::str::from_utf8(file.name).unwrap();
                trace!("loading {}...", name);
                match Server::load(file.data) {
                    Ok(server) => {
                        SERVERS.lock().push(server);
                    }
                    Err(e) => {
                        error!("failed to load server: {:?}", e);
                    }
                }
                trace!("loaded {}", name);
            }
        }
    }
}
