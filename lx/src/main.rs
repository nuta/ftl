#![no_std]
#![no_main]

use core::arch::naked_asm;
use core::mem::size_of;

use ftl::info;
use ftl::syscall::thread_create;
use ftl::syscall::thread_start;
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
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;

const PAGE_SIZE: usize = 4096; // TODO: system call?
const STACK_START: usize = 0x0200_0000;
const STACK_SIZE: usize = 256 * 1024;

#[repr(C, align(8))]
struct Aligned<const N: usize>([u8; N]);

static HELLO_ELF: Aligned<{ include_bytes!("../../initfs/bin/hello").len() }> =
    Aligned(*include_bytes!("../../initfs/bin/hello"));

const SYS_WRITE: usize = 1;
const ENOSYS: isize = 38;

extern "C" fn handle_syscall(
    arg0: usize,
    arg1: usize,
    arg2: usize,
    _arg3: usize,
    _arg4: usize,
    _arg5: usize,
    n: usize,
) -> isize {
    info!("syscall: n={}, [{:#x}, {:#x}, {:#x}]", n, arg0, arg1, arg2);
    if n == SYS_WRITE {
        let bytes = unsafe { core::slice::from_raw_parts(arg1 as *const u8, arg2) };
        ftl::syscall::print(bytes);
        arg2 as isize
    } else {
        -ENOSYS
    }
}

#[unsafe(naked)]
extern "C" fn syscall_handler() -> ! {
    naked_asm!(
        // Save caller-saved registers except for syscall-related ones
        // (rax, rcx, r11).
        "push rdi",
        "push rsi",
        "push rdx",
        "push r8",
        "push r9",
        "push r10",
        "push rbx",

        // Align the stack to 16 bytes.
        "mov rbx, rsp",
        "and rsp, -16",

        "mov rcx, r10", // arg3
        "sub rsp, 8", // Padding to keep it 16-bytes aligned
        "push rax", // syscall number (the last argument)
        "call {handle_syscall}",

        // Restore the stack pointer, and others.
        "mov rsp, rbx",
        "pop rbx",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r11", // user RFLAGS (from syscall frame)
        "pop rcx", // user RIP (from syscall frame)

        // Restore user RFLAGS.
        "push r11",
        "popfq",

        // Restore user RSP.
        "lea rsp, [rsp + 128]", // red zone

        // Go back to the application code.
        "jmp rcx",
        handle_syscall = sym handle_syscall,
    )
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

#[unsafe(no_mangle)]
fn main() {
    let root_isolate = HandleId::new(1);
    let root_vmspace = HandleId::new(2);

    let vmspace = vmspace_clone(root_vmspace).unwrap();
    let entry = load_elf(vmspace, &HELLO_ELF.0);
    let stack = vmo_create(STACK_SIZE).unwrap();
    vmspace_map(
        vmspace,
        stack,
        STACK_START,
        PageAttrs::READ | PageAttrs::WRITE,
    )
    .unwrap();

    // TODO: implement argc, argv, envp, auxv
    let sp = align_down(STACK_START + STACK_SIZE - 5 * size_of::<usize>(), 16);

    let fault_pc = syscall_handler as *const () as usize;
    let thread = thread_create(root_isolate, vmspace, entry, sp, fault_pc).unwrap();
    thread_start(thread).unwrap();

    ftl::info!("started hello at {:#x}", entry);
}
