mod accept;
mod accept4;
mod arch_prctl;
mod bind;
mod brk;
mod clock_gettime;
mod clock_nanosleep;
mod close;
mod dup2;
mod dup3;
mod epoll_create1;
mod epoll_ctl;
mod epoll_pwait;
mod epoll_wait;
mod eventfd;
mod eventfd2;
mod execve;
mod exit_group;
mod fcntl;
mod fork;
mod getpid;
mod getrandom;
mod kill;
mod listen;
mod lseek;
mod mmap;
mod pipe;
mod pipe2;
mod poll;
mod read;
mod recvfrom;
mod rt_sigaction;
mod rt_sigreturn;
mod sendto;
mod set_tid_address;
mod setsockopt;
mod socket;
mod wait4;
mod write;
mod writev;

use self::accept::sys_accept;
use self::accept4::sys_accept4;
use self::arch_prctl::sys_arch_prctl;
use self::bind::sys_bind;
use self::brk::sys_brk;
use self::clock_gettime::sys_clock_gettime;
use self::clock_nanosleep::sys_clock_nanosleep;
use self::close::sys_close;
use self::dup2::sys_dup2;
use self::dup3::sys_dup3;
use self::epoll_create1::sys_epoll_create1;
use self::epoll_ctl::sys_epoll_ctl;
use self::epoll_pwait::sys_epoll_pwait;
use self::epoll_wait::sys_epoll_wait;
use self::eventfd::sys_eventfd;
use self::eventfd2::sys_eventfd2;
use self::execve::sys_execve;
use self::exit_group::sys_exit_group;
use self::fcntl::sys_fcntl;
use self::fork::sys_fork;
use self::getpid::sys_getpid;
use self::getrandom::sys_getrandom;
use self::kill::sys_kill;
use self::listen::sys_listen;
use self::lseek::sys_lseek;
use self::mmap::sys_mmap;
use self::pipe::sys_pipe;
use self::pipe2::sys_pipe2;
use self::poll::sys_poll;
use self::read::sys_read;
use self::recvfrom::sys_recvfrom;
use self::rt_sigaction::sys_rt_sigaction;
use self::rt_sigreturn::sys_rt_sigreturn;
use self::sendto::sys_sendto;
use self::set_tid_address::sys_set_tid_address;
use self::setsockopt::sys_setsockopt;
use self::socket::sys_socket;
use self::wait4::sys_wait4;
use self::write::sys_write;
use self::writev::sys_writev;
use crate::arch::SyscallFrame;
use crate::thread::LxThread;
use crate::types;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_unsigned;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::off_t;
use crate::types::sys::epoll::EpollEvent;
use crate::types::sys::poll::PollFd;
use crate::types::sys::poll::nfds_t;
use crate::types::sys::syscall::SYS_ACCEPT;
use crate::types::sys::syscall::SYS_ACCEPT4;
use crate::types::sys::syscall::SYS_ARCH_PRCTL;
use crate::types::sys::syscall::SYS_BIND;
use crate::types::sys::syscall::SYS_BRK;
use crate::types::sys::syscall::SYS_CLOCK_GETTIME;
use crate::types::sys::syscall::SYS_CLOCK_NANOSLEEP;
use crate::types::sys::syscall::SYS_CLOSE;
use crate::types::sys::syscall::SYS_DUP2;
use crate::types::sys::syscall::SYS_DUP3;
use crate::types::sys::syscall::SYS_EPOLL_CREATE1;
use crate::types::sys::syscall::SYS_EPOLL_CTL;
use crate::types::sys::syscall::SYS_EPOLL_PWAIT;
use crate::types::sys::syscall::SYS_EPOLL_WAIT;
use crate::types::sys::syscall::SYS_EVENTFD;
use crate::types::sys::syscall::SYS_EVENTFD2;
use crate::types::sys::syscall::SYS_EXECVE;
use crate::types::sys::syscall::SYS_EXIT_GROUP;
use crate::types::sys::syscall::SYS_FCNTL;
use crate::types::sys::syscall::SYS_FORK;
use crate::types::sys::syscall::SYS_GETPID;
use crate::types::sys::syscall::SYS_GETRANDOM;
use crate::types::sys::syscall::SYS_KILL;
use crate::types::sys::syscall::SYS_LISTEN;
use crate::types::sys::syscall::SYS_LSEEK;
use crate::types::sys::syscall::SYS_MMAP;
use crate::types::sys::syscall::SYS_PIPE;
use crate::types::sys::syscall::SYS_PIPE2;
use crate::types::sys::syscall::SYS_POLL;
use crate::types::sys::syscall::SYS_READ;
use crate::types::sys::syscall::SYS_RECVFROM;
use crate::types::sys::syscall::SYS_RT_SIGACTION;
use crate::types::sys::syscall::SYS_RT_SIGRETURN;
use crate::types::sys::syscall::SYS_SENDTO;
use crate::types::sys::syscall::SYS_SET_TID_ADDRESS;
use crate::types::sys::syscall::SYS_SETSOCKOPT;
use crate::types::sys::syscall::SYS_SOCKET;
use crate::types::sys::syscall::SYS_WAIT4;
use crate::types::sys::syscall::SYS_WRITE;
use crate::types::sys::syscall::SYS_WRITEV;
use crate::types::sys::time::TimeSpec;
use crate::types::sys::uio::IoVec;

pub extern "C" fn handle_syscall(frame: *mut SyscallFrame) -> *mut SyscallFrame {
    // SAFETY: `syscall_handler` passes its register frame.
    let frame = unsafe { &mut *frame };
    let nr = frame.nr();
    let arg0 = frame.arg0();
    let arg1 = frame.arg1();
    let arg2 = frame.arg2();

    // SAFETY: The kernel returns the cookie we gave.
    let current = unsafe { LxThread::from_cookie(frame.cookie) };
    let result = match nr {
        SYS_WRITE => sys_write(&current, arg0 as c_int, arg1 as *const c_void, arg2),
        SYS_READ => sys_read(&current, arg0 as c_int, arg1 as *mut c_void, arg2),
        SYS_LSEEK => sys_lseek(&current, arg0 as c_int, arg1 as off_t, arg2 as c_int),
        SYS_RECVFROM => {
            sys_recvfrom(
                &current,
                arg0 as c_int,
                arg1 as *mut c_void,
                arg2,
                frame.arg3() as c_int,
                frame.arg4() as *mut u8,
                frame.arg5() as *mut u32,
            )
        }
        SYS_SENDTO => {
            sys_sendto(
                &current,
                arg0 as c_int,
                arg1 as *const c_void,
                arg2,
                frame.arg3() as c_int,
                frame.arg4() as *const u8,
                frame.arg5(),
            )
        }
        SYS_CLOSE => sys_close(&current, arg0 as c_int),
        SYS_DUP2 => sys_dup2(&current, arg0 as c_int, arg1 as c_int),
        SYS_DUP3 => sys_dup3(&current, arg0 as c_int, arg1 as c_int, arg2 as c_int),
        SYS_EPOLL_CREATE1 => sys_epoll_create1(&current, arg0 as c_int),
        SYS_EPOLL_CTL => {
            sys_epoll_ctl(
                &current,
                arg0 as c_int,
                arg1 as c_int,
                arg2 as c_int,
                frame.arg3() as *const EpollEvent,
            )
        }
        SYS_EPOLL_WAIT => {
            sys_epoll_wait(
                &current,
                arg0 as c_int,
                arg1 as *mut EpollEvent,
                arg2 as c_int,
                frame.arg3() as c_int,
            )
        }
        SYS_EPOLL_PWAIT => {
            sys_epoll_pwait(
                &current,
                arg0 as c_int,
                arg1 as *mut EpollEvent,
                arg2 as c_int,
                frame.arg3() as c_int,
                frame.arg4() as *const c_void,
            )
        }
        SYS_EVENTFD => sys_eventfd(&current, arg0 as c_unsigned),
        SYS_EVENTFD2 => sys_eventfd2(&current, arg0 as c_unsigned, arg1 as c_int),
        SYS_BRK => sys_brk(&current, arg0),
        SYS_MMAP => {
            sys_mmap(
                &current,
                arg0,
                arg1,
                arg2 as c_int,
                frame.arg3() as c_int,
                frame.arg4() as c_int,
                frame.arg5() as i64,
            )
        }
        SYS_PIPE => sys_pipe(&current, arg0 as *mut c_int),
        SYS_PIPE2 => sys_pipe2(&current, arg0 as *mut c_int, arg1 as c_int),
        SYS_POLL => sys_poll(&current, arg0 as *mut PollFd, arg1 as nfds_t, arg2 as c_int),
        SYS_WRITEV => sys_writev(&current, arg0 as c_int, arg1 as *const IoVec, arg2 as c_int),
        SYS_FORK => sys_fork(&current, frame),
        SYS_GETPID => sys_getpid(&current),
        SYS_GETRANDOM => sys_getrandom(&current, arg0 as *mut c_void, arg1, arg2 as c_unsigned),
        SYS_CLOCK_GETTIME => sys_clock_gettime(&current, arg0 as c_int, arg1 as *mut TimeSpec),
        SYS_CLOCK_NANOSLEEP => {
            sys_clock_nanosleep(
                &current,
                arg0 as c_int,
                arg1 as c_int,
                arg2 as *const TimeSpec,
                frame.arg3() as *mut TimeSpec,
            )
        }
        SYS_KILL => sys_kill(&current, arg0 as c_int, arg1 as c_int),
        SYS_RT_SIGACTION => {
            sys_rt_sigaction(
                &current,
                arg0 as c_int,
                arg1 as *const types::signal::SigAction,
                arg2 as *mut types::signal::SigAction,
                frame.arg3(),
            )
        }
        SYS_RT_SIGRETURN => sys_rt_sigreturn(&current, frame),
        SYS_SOCKET => sys_socket(&current, arg0 as c_int, arg1 as c_int, arg2 as c_int),
        SYS_BIND => sys_bind(&current, arg0 as c_int, arg1 as *const u8, arg2),
        SYS_LISTEN => sys_listen(&current, arg0 as c_int, arg1 as c_int),
        SYS_SETSOCKOPT => {
            sys_setsockopt(
                &current,
                arg0 as c_int,
                arg1 as c_int,
                arg2 as c_int,
                frame.arg3() as *const u8,
                frame.arg4(),
            )
        }
        SYS_ACCEPT => sys_accept(&current, arg0 as c_int, arg1 as *mut u8, arg2 as *mut u32),
        SYS_ACCEPT4 => {
            sys_accept4(
                &current,
                arg0 as c_int,
                arg1 as *mut u8,
                arg2 as *mut u32,
                frame.arg3() as c_int,
            )
        }
        SYS_EXECVE => {
            sys_execve(
                &current,
                arg0 as *const u8,
                arg1 as *const *const u8,
                arg2 as *const *const u8,
            )
        }
        SYS_WAIT4 => sys_wait4(&current, arg0 as c_int, arg1 as *mut c_int, arg2 as c_int),
        SYS_FCNTL => sys_fcntl(&current, arg0 as c_int, arg1 as c_int, arg2 as c_long),
        SYS_ARCH_PRCTL => sys_arch_prctl(&current, arg0 as c_int, arg1),
        SYS_SET_TID_ADDRESS => sys_set_tid_address(&current, arg0 as *mut c_int),
        SYS_EXIT_GROUP => sys_exit_group(&current, arg0 as c_int),
        _ => Err(Errno::ENOSYS),
    };

    let retval = match result {
        Ok(retval) => retval,
        Err(errno) => -(errno.as_int() as c_long),
    };
    frame.set_retval(retval);

    if nr != SYS_RT_SIGRETURN {
        // TODO: Nested signal handling is not supported yet.
        current.handle_pending_signal(frame);
    }

    frame
}
