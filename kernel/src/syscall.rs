use core::slice;

use ftl_types::error::ErrorCode;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_SEND;
use ftl_types::syscall::SYS_CONSOLE_WRITE;
use ftl_types::syscall::SYS_DMABUF_ALLOC;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_PCI_GET_BAR;
use ftl_types::syscall::SYS_PCI_LOOKUP;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_PCI_SET_BUSMASTER;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_X64_IOPL;

use crate::arch;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;
use crate::thread::return_to_user;

fn do_syscall(
    thread: &SharedRef<Thread>,
    n: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
) -> Result<usize, ErrorCode> {
    match n {
        SYS_CONSOLE_WRITE => {
            // FIXME: Use UserSlice.
            let s = unsafe { slice::from_raw_parts(a0 as *const u8, a1) };
            arch::console_write(s);
            Ok(0)
        }
        SYS_CHANNEL_CREATE => crate::channel::sys_channel_create(thread, a0),
        SYS_CHANNEL_SEND => crate::channel::sys_channel_send(thread, a0, a1, a2, a3, a4),
        SYS_DMABUF_ALLOC => crate::memory::sys_dmabuf_alloc(thread, a0, a1, a2),
        #[cfg(target_arch = "x86_64")]
        SYS_PCI_LOOKUP => arch::sys_pci_lookup(thread, a0, a1, a2, a3),
        #[cfg(target_arch = "x86_64")]
        SYS_PCI_SET_BUSMASTER => arch::sys_pci_set_busmaster(a0, a1, a2),
        #[cfg(target_arch = "x86_64")]
        SYS_PCI_GET_BAR => arch::sys_pci_get_bar(a0, a1, a2),
        #[cfg(target_arch = "x86_64")]
        SYS_X64_IOPL => arch::sys_x64_iopl(thread, a0),
        _ => {
            println!("unknown syscall: {}", n);
            Err(ErrorCode::UnknownSyscall)
        }
    }
}

pub extern "C" fn syscall_handler(
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    n: usize,
) -> ! {
    let current = &arch::get_cpuvar().current_thread;
    let thread = current.thread();
    let result = do_syscall(&thread, n, a0, a1, a2, a3, a4);
    unsafe { current.set_syscall_result(result) };
    return_to_user();
}
