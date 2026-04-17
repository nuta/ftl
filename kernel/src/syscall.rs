use ftl_types::error::ErrorCode;
use ftl_types::syscall::SYS_CHANNEL_CREATE;
use ftl_types::syscall::SYS_CHANNEL_DISCARD;
use ftl_types::syscall::SYS_CHANNEL_PEEK;
use ftl_types::syscall::SYS_CHANNEL_RECV;
use ftl_types::syscall::SYS_CHANNEL_SEND;
use ftl_types::syscall::SYS_CONSOLE_WRITE;
use ftl_types::syscall::SYS_DMABUF_ALLOC;
use ftl_types::syscall::SYS_HANDLE_CLOSE;
use ftl_types::syscall::SYS_INTERRUPT_ACKNOWLEDGE;
use ftl_types::syscall::SYS_INTERRUPT_ACQUIRE;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_PCI_GET_BAR;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_PCI_GET_INTERRUPT_LINE;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_PCI_GET_SUBSYSTEM_ID;
use ftl_types::syscall::SYS_PCI_LOOKUP;
#[cfg(target_arch = "x86_64")]
use ftl_types::syscall::SYS_PCI_SET_BUSMASTER;
use ftl_types::syscall::SYS_PROCESS_CREATE_INKERNEL;
use ftl_types::syscall::SYS_PROCESS_CREATE_SANDBOXED;
use ftl_types::syscall::SYS_PROCESS_EXIT;
use ftl_types::syscall::SYS_PROCESS_INJECT_HANDLE;
use ftl_types::syscall::SYS_SINK_ADD;
use ftl_types::syscall::SYS_SINK_CREATE;
use ftl_types::syscall::SYS_SINK_REMOVE;
use ftl_types::syscall::SYS_SINK_WAIT;
use ftl_types::syscall::SYS_THREAD_CREATE;
use ftl_types::syscall::SYS_THREAD_RESUME_WITH;
use ftl_types::syscall::SYS_THREAD_START;
use ftl_types::syscall::SYS_TIME_NOW;
use ftl_types::syscall::SYS_TIMER_CREATE;
use ftl_types::syscall::SYS_TIMER_SET;
use ftl_types::syscall::SYS_VMAREA_CREATE;
use ftl_types::syscall::SYS_VMAREA_READ;
use ftl_types::syscall::SYS_VMAREA_WRITE;
use ftl_types::syscall::SYS_VMSPACE_CREATE;
use ftl_types::syscall::SYS_VMSPACE_MAP;
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
    Exit,
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
        SYS_CONSOLE_WRITE => crate::print::sys_console_write(thread, a0, a1),
        SYS_HANDLE_CLOSE => crate::handle::sys_handle_close(thread, a0),
        SYS_CHANNEL_CREATE => crate::channel::sys_channel_create(thread, a0),
        SYS_CHANNEL_SEND => crate::channel::sys_channel_send(thread, a0, a1, a2, a3, a4),
        SYS_CHANNEL_RECV => crate::channel::sys_channel_recv(thread, a0, a1, a2),
        SYS_CHANNEL_PEEK => crate::channel::sys_channel_peek(thread, a0, a1),
        SYS_CHANNEL_DISCARD => crate::channel::sys_channel_discard(thread, a0, a1),
        SYS_SINK_CREATE => crate::sink::sys_sink_create(thread),
        SYS_SINK_ADD => crate::sink::sys_sink_add(thread, a0, a1),
        SYS_SINK_REMOVE => crate::sink::sys_sink_remove(thread, a0, a1),
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
        SYS_PCI_GET_SUBSYSTEM_ID => arch::sys_pci_get_subsystem_id(a0, a1),
        #[cfg(target_arch = "x86_64")]
        SYS_X64_IOPL => arch::sys_x64_iopl(thread, a0),
        SYS_INTERRUPT_ACQUIRE => crate::interrupt::sys_interrupt_acquire(thread, a0),
        SYS_INTERRUPT_ACKNOWLEDGE => crate::interrupt::sys_interrupt_acknowledge(thread, a0),
        SYS_PROCESS_EXIT => crate::process::sys_process_exit(thread),
        SYS_PROCESS_INJECT_HANDLE => crate::process::sys_process_inject_handle(thread, a0, a1),
        SYS_TIME_NOW => crate::timer::sys_time_now(),
        SYS_TIMER_CREATE => crate::timer::sys_timer_create(thread),
        SYS_TIMER_SET => crate::timer::sys_timer_set(thread, a0, a1),
        SYS_VMSPACE_CREATE => crate::vmspace::sys_vmspace_create(thread),
        SYS_VMSPACE_MAP => crate::vmspace::sys_vmspace_map(thread, a0, a1, a2, a3),
        SYS_VMAREA_CREATE => crate::vmarea::sys_vmarea_create(thread, a0),
        SYS_VMAREA_READ => crate::vmarea::sys_vmarea_read(thread, a0, a1, a2, a3),
        SYS_VMAREA_WRITE => crate::vmarea::sys_vmarea_write(thread, a0, a1, a2, a3),
        SYS_PROCESS_CREATE_INKERNEL => {
            crate::process::sys_process_create_inkernel(thread, a0, a1, a2)
        }
        SYS_PROCESS_CREATE_SANDBOXED => {
            crate::process::sys_process_create_sandboxed(thread, a0, a1, a2)
        }
        SYS_THREAD_CREATE => crate::thread::sys_thread_create(thread, a0, a1, a2, a3),
        SYS_THREAD_START => crate::thread::sys_thread_start(thread, a0),
        SYS_THREAD_RESUME_WITH => crate::thread::sys_thread_resume_with(thread, a0, a1),
        _ => {
            trace!("unknown syscall: {}", n);
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
            unsafe { thread.set_syscall_result(Ok(retval)) };
        }
        Ok(SyscallResult::Blocked(promise)) => {
            thread.block_on(promise);
        }
        Ok(SyscallResult::Exit) => {
            // Do nothing and switch to another thread.
        }
        Err(error) => {
            unsafe { thread.set_syscall_result(Err(error)) };
        }
    }
    return_to_user();
}
