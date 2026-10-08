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
    let retval = match Syscall::from_usize(regs.n) {
        Some(Syscall::ThreadExit) => crate::thread::sys_thread_exit(thread, &regs),
        Some(Syscall::VmoCreateZeroed) => crate::vmobject::sys_vmo_create_zeroed(&thread, &regs),
        Some(Syscall::VmoRead) => crate::vmobject::sys_vmo_read(&thread, &regs),
        Some(Syscall::VmoWrite) => crate::vmobject::sys_vmo_write(&thread, &regs),
        Some(Syscall::VmoCreateUser) => crate::vmobject::sys_vmo_create_user(&thread, &regs),
        Some(Syscall::VmoSupply) => crate::vmobject::sys_vmo_supply(&thread, &regs),
        Some(Syscall::VmoSnapshot) => crate::vmobject::sys_vmo_snapshot(&thread, &regs),
        Some(Syscall::VmSpaceClone) => crate::vmspace::sys_vmspace_clone(&thread, &regs),
        Some(Syscall::VmSpaceMap) => crate::vmspace::sys_vmspace_map(&thread, &regs),
        Some(Syscall::VmSpaceUnmap) => crate::vmspace::sys_vmspace_unmap(&thread, &regs),
        Some(Syscall::VmSpacePermit) => crate::vmspace::sys_vmspace_permit(&thread, &regs),
        Some(Syscall::ThreadCreate) => crate::thread::sys_thread_create(&thread, &regs),
        Some(Syscall::ThreadStart) => crate::thread::sys_thread_start(&thread, &regs),
        Some(Syscall::ThreadSubscribe) => crate::thread::sys_thread_subscribe(&thread, &regs),
        Some(Syscall::ThreadWriteRegs) => {
            crate::thread::sys_thread_write_regs(&thread, arch_thread, &regs)
        }
        Some(Syscall::ThreadCopyRegs) => {
            crate::thread::sys_thread_copy_regs(&thread, arch_thread, &regs)
        }
        Some(Syscall::PollCreate) => crate::poll::sys_poll_create(&thread, &regs),
        Some(Syscall::PollWait) => {
            crate::poll::sys_poll_wait(&thread, &cpuvar.current_thread, &regs)
        }
        Some(Syscall::PollWaitUntil) => {
            crate::poll::sys_poll_wait_until(&thread, &cpuvar.current_thread, &regs)
        }
        Some(Syscall::PollNotify) => crate::poll::sys_poll_notify(&thread, &regs),
        Some(Syscall::NetCreate) => crate::net::sys_net_create(&thread, &regs),
        Some(Syscall::NetSubscribe) => crate::net::sys_net_subscribe(&thread, &regs),
        Some(Syscall::NetBind) => crate::net::sys_net_bind(&thread, &regs),
        Some(Syscall::NetUnbind) => crate::net::sys_net_unbind(&thread, &regs),
        Some(Syscall::NetRecv) => crate::net::sys_net_recv(&thread, &regs),
        Some(Syscall::NetSend) => crate::net::sys_net_send(&thread, &regs),
        Some(Syscall::HandleClose) => crate::handle::sys_handle_close(&thread, &regs),
        Some(Syscall::MonoTimeRead) => crate::timer::sys_monotime_read(&thread, &regs),
        Some(Syscall::WallTimeRead) => crate::timer::sys_walltime_read(&thread, &regs),
        Some(Syscall::RandomRead) => crate::random::sys_random_read(&thread, &regs),
        Some(Syscall::ConsoleOpen) => crate::console::sys_console_open(&thread, &regs),
        Some(Syscall::ConsoleWrite) => crate::console::sys_console_write(&thread, &regs),
        Some(Syscall::ConsoleRead) => crate::console::sys_console_read(&thread, &regs),
        Some(Syscall::ConsoleSubscribe) => crate::console::sys_console_subscribe(&thread, &regs),
        None => Err(ErrorCode::UnknownSyscall),
    };

    let retval = match retval {
        Ok(SyscallOutput::Done(retval)) if retval > isize::MAX as usize => {
            // TODO: Prevent this.
            error!("syscall {} returned too large value: {:#x}", regs.n, retval);
            ErrorCode::OutOfBounds.as_usize()
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
