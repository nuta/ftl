use ftl_types::error::ErrorCode;
use ftl_types::syscall::Syscall;

use crate::arch::get_cpuvar;
use crate::scheduler;

pub enum SyscallOutput {
    Done(usize),
    Blocked,
    Exited,
}

fn do_handle_syscall() {
    let cpuvar = get_cpuvar();
    let thread = cpuvar.current_thread.thread().unwrap();
    // TODO: safety
    let arch_thread = unsafe { &mut *thread.arch().get() };
    let regs = arch_thread.get_syscall_regs();
    let retval = match regs.n {
        n if n == Syscall::Print as usize => {
            // TODO: syscall retval
            crate::print::sys_print(&regs)
        }
        n if n == Syscall::ThreadExit as usize => crate::thread::sys_thread_exit(&thread, &regs),
        n if n == Syscall::VmoCreate as usize => crate::vmobject::sys_vmo_create(&thread, &regs),
        n if n == Syscall::VmoWrite as usize => crate::vmobject::sys_vmo_write(&thread, &regs),
        n if n == Syscall::VmSpaceClone as usize => {
            crate::vmspace::sys_vmspace_clone(&thread, &regs)
        }
        n if n == Syscall::VmSpaceMap as usize => crate::vmspace::sys_vmspace_map(&thread, &regs),
        n if n == Syscall::ThreadCreate as usize => {
            crate::thread::sys_thread_create(&thread, &regs)
        }
        n if n == Syscall::ThreadStart as usize => crate::thread::sys_thread_start(&thread, &regs),
        n if n == Syscall::ThreadWriteRegs as usize => {
            crate::thread::sys_thread_write_regs(&thread, arch_thread, &regs)
        }
        n if n == Syscall::ThreadCopyRegs as usize => {
            crate::thread::sys_thread_copy_regs(&thread, arch_thread, &regs)
        }
        n if n == Syscall::PollCreate as usize => crate::poll::sys_poll_create(&thread, &regs),
        n if n == Syscall::PollWait as usize => {
            crate::poll::sys_poll_wait(&thread, &cpuvar.current_thread, &regs)
        }
        n if n == Syscall::PollNotify as usize => crate::poll::sys_poll_notify(&thread, &regs),
        n if n == Syscall::NetCreate as usize => crate::net::sys_net_create(&thread, &regs),
        n if n == Syscall::NetSubscribe as usize => crate::net::sys_net_subscribe(&thread, &regs),
        n if n == Syscall::NetBind as usize => crate::net::sys_net_bind(&thread, &regs),
        n if n == Syscall::NetUnbind as usize => crate::net::sys_net_unbind(&thread, &regs),
        n if n == Syscall::NetPeek as usize => crate::net::sys_net_peek(&thread, &regs),
        n if n == Syscall::NetRecv as usize => crate::net::sys_net_recv(&thread, &regs),
        n if n == Syscall::NetDrop as usize => crate::net::sys_net_drop(&thread, &regs),
        n if n == Syscall::NetSend as usize => crate::net::sys_net_send(&thread, &regs),
        n if n == Syscall::HandleClose as usize => crate::handle::sys_handle_close(&thread, &regs),
        _ => Err(ErrorCode::Unsupported),
    };

    let retval = match retval {
        Ok(SyscallOutput::Done(retval)) if retval > isize::MAX as usize => {
            // TODO: Prevent this.
            unreachable!();
        }
        Ok(SyscallOutput::Blocked) => return,
        Ok(SyscallOutput::Done(retval)) => retval,
        Ok(SyscallOutput::Exited) => return,
        Err(err) => err.as_usize(),
    };

    arch_thread.set_syscall_retval(retval);
}

pub extern "C" fn handle_syscall() -> ! {
    // `return_to_user` won't return. To make sure all objects are dropped,
    // do not add any more logic to this function.
    do_handle_syscall();
    scheduler::return_to_user();
}
