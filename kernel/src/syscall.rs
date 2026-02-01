use core::slice;

use ftl_types::error::ErrorCode;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_OOL_READ;
use ftl_types::syscall::SYS_CHANNEL_OOL_WRITE;
use ftl_types::syscall::SYS_CHANNEL_SEND;
use ftl_types::syscall::SYS_CONSOLE_WRITE;
use ftl_types::syscall::SYS_DMABUF_ALLOC;
use ftl_types::syscall::SYS_INTERRUPT_ACKNOWLEDGE;
use ftl_types::syscall::SYS_INTERRUPT_ACQUIRE;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_PCI_GET_BAR;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_PCI_GET_INTERRUPT_LINE;
use ftl_types::syscall::SYS_PCI_LOOKUP;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_PCI_SET_BUSMASTER;
use ftl_types::syscall::SYS_SINK_ADD;
use ftl_types::syscall::SYS_SINK_CREATE;
use ftl_types::syscall::SYS_SINK_WAIT;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_X64_IOPL;

use crate::arch;
use crate::shared_ref::SharedRef;
use crate::thread::Promise;
use crate::thread::Thread;
use crate::thread::return_to_user;

pub enum SyscallResult {
    Return(usize),
    Blocked(Promise),
}

fn do_syscall(
    thread: &SharedRef<Thread>,
    n: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
) -> Result<SyscallResult, ErrorCode> {
    match n {
        SYS_CONSOLE_WRITE => {
            // FIXME: Use UserSlice.
            let s = unsafe { slice::from_raw_parts(a0 as *const u8, a1) };
            arch::console_write(s);
            Ok(SyscallResult::Return(0))
        }
        SYS_CHANNEL_CREATE => crate::channel::sys_channel_create(thread, a0),
        SYS_CHANNEL_SEND => crate::channel::sys_channel_send(thread, a0, a1, a2, a3, a4),
        SYS_CHANNEL_OOL_READ => crate::channel::sys_channel_ool_read(thread, a0, a1, a2, a3, a4),
        SYS_CHANNEL_OOL_WRITE => crate::channel::sys_channel_ool_write(thread, a0, a1, a2, a3, a4),
        SYS_SINK_CREATE => crate::sink::sys_sink_create(thread),
        SYS_SINK_ADD => crate::sink::sys_sink_add(thread, a0, a1),
        SYS_SINK_WAIT => crate::sink::sys_sink_wait(thread, a0, a1),
        SYS_DMABUF_ALLOC => crate::memory::sys_dmabuf_alloc(thread, a0, a1, a2),
        #[cfg(target_arch = "x86_64")]
        SYS_PCI_LOOKUP => arch::sys_pci_lookup(thread, a0, a1, a2, a3),
        #[cfg(target_arch = "x86_64")]
        SYS_PCI_SET_BUSMASTER => arch::sys_pci_set_busmaster(a0, a1, a2),
        #[cfg(target_arch = "x86_64")]
        SYS_PCI_GET_BAR => arch::sys_pci_get_bar(a0, a1, a2),
        #[cfg(target_arch = "x86_64")]
        SYS_PCI_GET_INTERRUPT_LINE => arch::sys_pci_get_interrupt_line(a0, a1),
        #[cfg(target_arch = "x86_64")]
        SYS_X64_IOPL => arch::sys_x64_iopl(thread, a0),
        SYS_INTERRUPT_ACQUIRE => crate::interrupt::sys_interrupt_acquire(thread, a0),
        SYS_INTERRUPT_ACKNOWLEDGE => crate::interrupt::sys_interrupt_acknowledge(thread, a0),
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
    match do_syscall(&thread, n, a0, a1, a2, a3, a4) {
        Ok(SyscallResult::Return(retval)) => {
            unsafe { current.set_syscall_result(Ok(retval)) };
        }
        Ok(SyscallResult::Blocked(promise)) => {
            thread.block_on(promise);
        }
        Err(error) => {
            unsafe { current.set_syscall_result(Err(error)) };
        }
    }
    return_to_user();
}
