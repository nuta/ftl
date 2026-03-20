use core::mem::size_of;
use core::slice;

use ftl::arch;
use ftl::channel::Channel;
use ftl::prelude::trace;
use ftl::process::PROCESS_NAME_MAX_LEN;
use ftl::process::Process;
use ftl::thread::Thread;
use ftl::vmarea::VmArea;
use ftl::vmspace::PageAttrs;
use ftl::vmspace::VmSpace;
use ftl_types::start_info::StartInfo;
use ftl_utils::alignment::align_up;

use crate::elf::Elf;
use crate::initfs;
use crate::initfs::InitFs;

const STACK_SIZE: usize = 1024 * 1024;

fn map_elf(vmspace: &VmSpace, file: &initfs::File, elf: &Elf<'_>, image_size: usize) {
    let vmarea = VmArea::new(image_size).expect("failed to create image vmarea");
    vmspace
        .map(&vmarea, 0, PageAttrs::READABLE | PageAttrs::WRITABLE)
        .expect("failed to map image vmarea");

    for phdr in elf.phdrs {
        if !phdr.is_load() || phdr.filesz == 0 {
            continue;
        }

        let start = phdr.offset as usize;
        let end = start + phdr.filesz as usize;
        vmarea
            .write(phdr.vaddr as usize, &file.data[start..end])
            .unwrap();
    }
}

fn map_stack(vmspace: &VmSpace, stack_bottom: usize) -> usize {
    let stack_vmarea = VmArea::new(STACK_SIZE).expect("failed to create stack vmarea");
    vmspace
        .map(
            &stack_vmarea,
            stack_bottom,
            PageAttrs::READABLE | PageAttrs::WRITABLE,
        )
        .expect("failed to map stack vmarea");

    stack_bottom + STACK_SIZE
}

fn map_start_info(vmspace: &VmSpace, page_size: usize, app_name: &str, start_info_addr: usize) {
    let inherited = arch::start_info();
    let src = app_name.as_bytes();
    let name_len = src.len().min(PROCESS_NAME_MAX_LEN);
    let mut name_bytes = [0; PROCESS_NAME_MAX_LEN];
    name_bytes[..name_len].copy_from_slice(&src[..name_len]);
    let start_info = StartInfo {
        syscall: inherited.syscall,
        min_page_size: inherited.min_page_size,
        name: name_bytes,
        name_len: name_len as u8,
        initfs_ptr: core::ptr::null(),
        initfs_size: 0,
    };

    let vmarea = VmArea::new(page_size).expect("failed to create start-info vmarea");
    let bytes = unsafe {
        slice::from_raw_parts(
            (&start_info as *const StartInfo).cast::<u8>(),
            size_of::<StartInfo>(),
        )
    };
    vmarea.write(0, bytes).expect("failed to write start info");

    vmspace
        .map(
            &vmarea,
            start_info_addr,
            PageAttrs::READABLE | PageAttrs::WRITABLE,
        )
        .expect("failed to map start info");
}

pub fn load_app(file: &initfs::File) -> Channel {
    let app_name = file.name;
    let page_size = arch::min_page_size();
    let elf = Elf::parse(file.data).expect("failed to parse ELF");
    let image_size = align_up(elf.image_size, page_size);
    let stack_bottom = align_up(image_size + page_size, page_size);
    let start_info_addr = align_up(stack_bottom + STACK_SIZE + page_size, page_size);

    let vmspace = VmSpace::new().expect("failed to create vmspace");
    map_elf(&vmspace, file, &elf, image_size);
    let sp = map_stack(&vmspace, stack_bottom);
    map_start_info(&vmspace, page_size, app_name, start_info_addr);

    let process = Process::create_inkernel(&vmspace, file.name).expect("failed to create process");
    let (our_ch, their_ch) = Channel::new().expect("failed to create control channel");
    let their_id = process
        .inject_handle(their_ch)
        .expect("failed to inject control channel handle");
    assert_eq!(their_id.as_usize(), 1);

    let thread = Thread::create(&process, elf.entry, sp, start_info_addr)
        .expect("failed to create initial thread");
    thread.start().expect("failed to start thread");
    our_ch
}
