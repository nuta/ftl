use alloc::boxed::Box;
use core::mem::MaybeUninit;

use ftl_arrayvec::ArrayString;
use ftl_arrayvec::ArrayVec;
use ftl_types::environ::StartInfo;

use crate::address::PAddr;
use crate::arch;
use crate::cpuvar;
use crate::isolation::InKernelIsolation;
use crate::memory;
use crate::memory::PAGE_ALLOCATOR;
use crate::process::Process;
use crate::thread::Thread;
use crate::thread::return_to_user;
use crate::vmspace::VmSpace;

pub struct FreeRam {
    pub start: PAddr,
    pub end: PAddr,
}

pub struct BootInfo {
    pub free_rams: ArrayVec<FreeRam, 8>,
    pub initfs: &'static [u8],
}

#[repr(align(4096))]
struct PageAligned<T>(T);

const BOOTSTRAP_IMAGE: PageAligned<[u8; include_bytes!("../../bootstrap.bin").len()]> =
    PageAligned(*include_bytes!("../../bootstrap.bin"));

fn create_bootstrap_process(bootinfo: &BootInfo) {
    const STACK_SIZE: usize = 1024 * 1024;

    let base_addr = BOOTSTRAP_IMAGE.0.as_ptr() as usize;

    // Allocate stack.
    let stack_bottom_paddr = PAGE_ALLOCATOR
        .alloc(STACK_SIZE)
        .expect("failed to allocate stack");
    let stack_bottom_vaddr = arch::paddr2vaddr(stack_bottom_paddr);
    let sp = stack_bottom_vaddr.as_usize() + STACK_SIZE;

    let bootstrap_name = ArrayString::from_static("bootstrap");

    let info_uninit = Box::leak(Box::new(MaybeUninit::<StartInfo>::uninit()));
    info_uninit.write(StartInfo {
        syscall: arch::direct_syscall_handler,
        min_page_size: arch::MIN_PAGE_SIZE,
        name: [
            b'b', b'o', b'o', b't', b's', b't', b'r', b'a', b'p', 0, 0, 0, 0, 0, 0,
        ],
        name_len: 10,
        initfs_ptr: bootinfo.initfs.as_ptr(),
        initfs_size: bootinfo.initfs.len(),
    });
    let start_info = info_uninit.as_ptr() as usize;

    trace!("bootstrap: base={:x}, sp={:x}", base_addr, sp);
    let vmspace = VmSpace::new().unwrap();
    let isolation = InKernelIsolation::new(vmspace).unwrap();
    let process = Process::new(bootstrap_name, isolation).expect("failed to create process");
    let thread = Thread::new(process, base_addr, sp, start_info).expect("failed to create thread");
    thread.start();
}

pub fn boot(bootinfo: &BootInfo) -> ! {
    memory::init(bootinfo);
    cpuvar::init();
    create_bootstrap_process(bootinfo);
    return_to_user();
}
