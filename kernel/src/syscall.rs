use ftl_types::syscall::Syscall;

use crate::arch::get_cpuvar;
use crate::scheduler;

pub enum SyscallOutput {
    Done(usize),
}

fn do_handle_syscall() {
    let cpuvar = get_cpuvar();
    let vcpu = cpuvar.current_vcpu.vcpu().unwrap();
    // TODO: safety
    let arch_vcpu = unsafe { &mut *vcpu.arch().get() };
    let regs = arch_vcpu.get_syscall_regs();
    let retval = match regs.n {
        n if n == Syscall::Print as usize => {
            // TODO: syscall retval
            crate::print::sys_print(&regs)
        }
        n if n == Syscall::VCpuExit as usize => {
            todo!()
        }
        n if n == Syscall::VmoCreate as usize => crate::vmobject::sys_vmo_create(&vcpu, &regs),
        _ => todo!(),
    };

    let retval = match retval {
        Ok(SyscallOutput::Done(retval)) if retval > isize::MAX as usize => {
            // TODO: Prevent this.
            unreachable!();
        }
        Ok(SyscallOutput::Done(retval)) => retval,
        Err(err) => err.as_usize(),
    };

    arch_vcpu.set_syscall_retval(retval);
}

pub extern "C" fn handle_syscall() -> ! {
    // `return_to_user` won't return. To make sure all objects are dropped,
    // do not add any more logic to this function.
    do_handle_syscall();
    scheduler::return_to_user();
}
