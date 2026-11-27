mod accept;
mod accept4;
mod access;
mod arch_prctl;
mod bind;
mod brk;
mod capget;
mod chdir;
mod chmod;
mod clock_getres;
mod clock_gettime;
mod clock_nanosleep;
mod clone;
mod close;
mod connect;
mod creat;
mod dup2;
mod dup3;
mod epoll_create1;
mod epoll_ctl;
mod epoll_pwait;
mod epoll_wait;
mod eventfd;
mod eventfd2;
mod execve;
mod exit;
mod exit_group;
mod faccessat;
mod fchmod;
mod fchmodat;
mod fcntl;
mod fork;
mod fstat;
mod fsync;
mod ftruncate;
mod futex;
mod getcwd;
mod getdents64;
mod getgroups;
mod getpeername;
mod getpid;
mod getppid;
mod getpriority;
mod getrandom;
mod getsockname;
mod getsockopt;
mod gettid;
mod getuid;
mod ioctl;
mod kill;
mod link;
mod linkat;
mod listen;
mod lseek;
mod lstat;
mod madvise;
mod mkdir;
mod mkdirat;
mod mmap;
mod mprotect;
mod munmap;
mod nanosleep;
mod newfstatat;
mod open;
mod openat;
mod pipe;
mod pipe2;
mod poll;
mod prctl;
mod pread64;
mod prlimit64;
mod pselect6;
mod read;
mod readlink;
mod readv;
mod recvfrom;
mod rmdir;
mod rt_sigaction;
mod rt_sigprocmask;
mod rt_sigreturn;
mod sched_getaffinity;
mod sched_yield;
mod sendto;
mod set_robust_list;
mod set_tid_address;
mod setsockopt;
mod shutdown;
mod sigaltstack;
mod socket;
mod stat;
mod statx;
mod sync;
mod sysinfo;
mod tgkill;
mod truncate;
mod umask;
mod uname;
mod unlink;
mod unlinkat;
mod utimensat;
mod wait4;
mod write;
mod writev;

use ftl_types::thread::ExitReason;
use ftl_types::thread::FaultFrame;

use self::accept::sys_accept;
use self::accept4::sys_accept4;
use self::access::sys_access;
use self::arch_prctl::sys_arch_prctl;
use self::bind::sys_bind;
use self::brk::sys_brk;
use self::capget::sys_capget;
use self::chdir::sys_chdir;
use self::chmod::sys_chmod;
use self::clock_getres::sys_clock_getres;
use self::clock_gettime::sys_clock_gettime;
use self::clock_nanosleep::sys_clock_nanosleep;
use self::clone::sys_clone;
use self::close::sys_close;
use self::connect::sys_connect;
use self::creat::sys_creat;
use self::dup2::sys_dup2;
use self::dup3::sys_dup3;
use self::epoll_create1::sys_epoll_create1;
use self::epoll_ctl::sys_epoll_ctl;
use self::epoll_pwait::sys_epoll_pwait;
use self::epoll_wait::sys_epoll_wait;
use self::eventfd::sys_eventfd;
use self::eventfd2::sys_eventfd2;
use self::execve::sys_execve;
use self::exit::sys_exit;
use self::exit_group::sys_exit_group;
use self::faccessat::sys_faccessat;
use self::fchmod::sys_fchmod;
use self::fchmodat::sys_fchmodat;
use self::fcntl::sys_fcntl;
use self::fork::sys_fork;
use self::fstat::sys_fstat;
use self::fsync::sys_fsync;
use self::ftruncate::sys_ftruncate;
use self::futex::sys_futex;
use self::getcwd::sys_getcwd;
use self::getdents64::sys_getdents64;
use self::getgroups::sys_getgroups;
use self::getpeername::sys_getpeername;
use self::getpid::sys_getpid;
use self::getppid::sys_getppid;
use self::getpriority::sys_getpriority;
use self::getrandom::sys_getrandom;
use self::getsockname::sys_getsockname;
use self::getsockopt::sys_getsockopt;
use self::gettid::sys_gettid;
use self::getuid::sys_getuid;
use self::ioctl::sys_ioctl;
use self::kill::sys_kill;
use self::link::sys_link;
use self::linkat::sys_linkat;
use self::listen::sys_listen;
use self::lseek::sys_lseek;
use self::lstat::sys_lstat;
use self::madvise::sys_madvise;
use self::mkdir::sys_mkdir;
use self::mkdirat::sys_mkdirat;
use self::mmap::sys_mmap;
use self::mprotect::sys_mprotect;
use self::munmap::sys_munmap;
use self::nanosleep::sys_nanosleep;
use self::newfstatat::sys_newfstatat;
use self::open::sys_open;
use self::openat::sys_openat;
use self::pipe::sys_pipe;
use self::pipe2::sys_pipe2;
use self::poll::sys_poll;
use self::prctl::sys_prctl;
use self::pread64::sys_pread64;
use self::prlimit64::sys_prlimit64;
use self::pselect6::sys_pselect6;
use self::read::sys_read;
use self::readlink::sys_readlink;
use self::readv::sys_readv;
use self::recvfrom::sys_recvfrom;
use self::rmdir::sys_rmdir;
use self::rt_sigaction::sys_rt_sigaction;
use self::rt_sigprocmask::sys_rt_sigprocmask;
use self::rt_sigreturn::sys_rt_sigreturn;
use self::sched_getaffinity::sys_sched_getaffinity;
use self::sched_yield::sys_sched_yield;
use self::sendto::sys_sendto;
use self::set_robust_list::sys_set_robust_list;
use self::set_tid_address::sys_set_tid_address;
use self::setsockopt::sys_setsockopt;
use self::shutdown::sys_shutdown;
use self::sigaltstack::sys_sigaltstack;
use self::socket::sys_socket;
use self::stat::sys_stat;
use self::statx::sys_statx;
use self::sync::sys_sync;
use self::sysinfo::sys_sysinfo;
use self::tgkill::sys_tgkill;
use self::truncate::sys_truncate;
use self::umask::sys_umask;
use self::uname::sys_uname;
use self::unlink::sys_unlink;
use self::unlinkat::sys_unlinkat;
use self::utimensat::sys_utimensat;
use self::wait4::sys_wait4;
use self::write::sys_write;
use self::writev::sys_writev;
use crate::arch::SyscallFrame;
use crate::arch::UserContext;
use crate::thread::Thread;
use crate::types;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::c_ulong;
use crate::types::c_unsigned;
use crate::types::c_void;
use crate::types::errno::Errno;
use crate::types::off_t;
use crate::types::sys::capability::CapUserData;
use crate::types::sys::capability::CapUserHeader;
use crate::types::sys::epoll::EpollEvent;
use crate::types::sys::poll::PollFd;
use crate::types::sys::poll::nfds_t;
use crate::types::sys::resource::RLimit;
use crate::types::sys::resource::RUsage;
use crate::types::sys::stat::Stat;
use crate::types::sys::stat::Statx;
use crate::types::sys::syscall::SYS_ACCEPT;
use crate::types::sys::syscall::SYS_ACCEPT4;
use crate::types::sys::syscall::SYS_ACCESS;
use crate::types::sys::syscall::SYS_ARCH_PRCTL;
use crate::types::sys::syscall::SYS_BIND;
use crate::types::sys::syscall::SYS_BRK;
use crate::types::sys::syscall::SYS_CAPGET;
use crate::types::sys::syscall::SYS_CHDIR;
use crate::types::sys::syscall::SYS_CHMOD;
use crate::types::sys::syscall::SYS_CLOCK_GETRES;
use crate::types::sys::syscall::SYS_CLOCK_GETTIME;
use crate::types::sys::syscall::SYS_CLOCK_NANOSLEEP;
use crate::types::sys::syscall::SYS_CLONE;
use crate::types::sys::syscall::SYS_CLOSE;
use crate::types::sys::syscall::SYS_CONNECT;
use crate::types::sys::syscall::SYS_CREAT;
use crate::types::sys::syscall::SYS_DUP2;
use crate::types::sys::syscall::SYS_DUP3;
use crate::types::sys::syscall::SYS_EPOLL_CREATE1;
use crate::types::sys::syscall::SYS_EPOLL_CTL;
use crate::types::sys::syscall::SYS_EPOLL_PWAIT;
use crate::types::sys::syscall::SYS_EPOLL_WAIT;
use crate::types::sys::syscall::SYS_EVENTFD;
use crate::types::sys::syscall::SYS_EVENTFD2;
use crate::types::sys::syscall::SYS_EXECVE;
use crate::types::sys::syscall::SYS_EXIT;
use crate::types::sys::syscall::SYS_EXIT_GROUP;
use crate::types::sys::syscall::SYS_FACCESSAT;
use crate::types::sys::syscall::SYS_FCHMOD;
use crate::types::sys::syscall::SYS_FCHMODAT;
use crate::types::sys::syscall::SYS_FCNTL;
use crate::types::sys::syscall::SYS_FDATASYNC;
use crate::types::sys::syscall::SYS_FORK;
use crate::types::sys::syscall::SYS_FSTAT;
use crate::types::sys::syscall::SYS_FSYNC;
use crate::types::sys::syscall::SYS_FTRUNCATE;
use crate::types::sys::syscall::SYS_FUTEX;
use crate::types::sys::syscall::SYS_GETCWD;
use crate::types::sys::syscall::SYS_GETDENTS64;
use crate::types::sys::syscall::SYS_GETEGID;
use crate::types::sys::syscall::SYS_GETEUID;
use crate::types::sys::syscall::SYS_GETGID;
use crate::types::sys::syscall::SYS_GETGROUPS;
use crate::types::sys::syscall::SYS_GETPEERNAME;
use crate::types::sys::syscall::SYS_GETPID;
use crate::types::sys::syscall::SYS_GETPPID;
use crate::types::sys::syscall::SYS_GETPRIORITY;
use crate::types::sys::syscall::SYS_GETRANDOM;
use crate::types::sys::syscall::SYS_GETSOCKNAME;
use crate::types::sys::syscall::SYS_GETSOCKOPT;
use crate::types::sys::syscall::SYS_GETTID;
use crate::types::sys::syscall::SYS_GETUID;
use crate::types::sys::syscall::SYS_IOCTL;
use crate::types::sys::syscall::SYS_KILL;
use crate::types::sys::syscall::SYS_LINK;
use crate::types::sys::syscall::SYS_LINKAT;
use crate::types::sys::syscall::SYS_LISTEN;
use crate::types::sys::syscall::SYS_LSEEK;
use crate::types::sys::syscall::SYS_LSTAT;
use crate::types::sys::syscall::SYS_MADVISE;
use crate::types::sys::syscall::SYS_MKDIR;
use crate::types::sys::syscall::SYS_MKDIRAT;
use crate::types::sys::syscall::SYS_MMAP;
use crate::types::sys::syscall::SYS_MPROTECT;
use crate::types::sys::syscall::SYS_MUNMAP;
use crate::types::sys::syscall::SYS_NANOSLEEP;
use crate::types::sys::syscall::SYS_NEWFSTATAT;
use crate::types::sys::syscall::SYS_OPEN;
use crate::types::sys::syscall::SYS_OPENAT;
use crate::types::sys::syscall::SYS_PIPE;
use crate::types::sys::syscall::SYS_PIPE2;
use crate::types::sys::syscall::SYS_POLL;
use crate::types::sys::syscall::SYS_PRCTL;
use crate::types::sys::syscall::SYS_PREAD64;
use crate::types::sys::syscall::SYS_PRLIMIT64;
use crate::types::sys::syscall::SYS_PSELECT6;
use crate::types::sys::syscall::SYS_READ;
use crate::types::sys::syscall::SYS_READLINK;
use crate::types::sys::syscall::SYS_READV;
use crate::types::sys::syscall::SYS_RECVFROM;
use crate::types::sys::syscall::SYS_RMDIR;
use crate::types::sys::syscall::SYS_RT_SIGACTION;
use crate::types::sys::syscall::SYS_RT_SIGPROCMASK;
use crate::types::sys::syscall::SYS_RT_SIGRETURN;
use crate::types::sys::syscall::SYS_SCHED_GETAFFINITY;
use crate::types::sys::syscall::SYS_SCHED_YIELD;
use crate::types::sys::syscall::SYS_SENDTO;
use crate::types::sys::syscall::SYS_SET_ROBUST_LIST;
use crate::types::sys::syscall::SYS_SET_TID_ADDRESS;
use crate::types::sys::syscall::SYS_SETSOCKOPT;
use crate::types::sys::syscall::SYS_SHUTDOWN;
use crate::types::sys::syscall::SYS_SIGALTSTACK;
use crate::types::sys::syscall::SYS_SOCKET;
use crate::types::sys::syscall::SYS_STAT;
use crate::types::sys::syscall::SYS_STATX;
use crate::types::sys::syscall::SYS_SYNC;
use crate::types::sys::syscall::SYS_SYSINFO;
use crate::types::sys::syscall::SYS_TGKILL;
use crate::types::sys::syscall::SYS_TRUNCATE;
use crate::types::sys::syscall::SYS_UMASK;
use crate::types::sys::syscall::SYS_UNAME;
use crate::types::sys::syscall::SYS_UNLINK;
use crate::types::sys::syscall::SYS_UNLINKAT;
use crate::types::sys::syscall::SYS_UTIMENSAT;
use crate::types::sys::syscall::SYS_VFORK;
use crate::types::sys::syscall::SYS_WAIT4;
use crate::types::sys::syscall::SYS_WRITE;
use crate::types::sys::syscall::SYS_WRITEV;
use crate::types::sys::sysinfo::SysInfo;
use crate::types::sys::time::TimeSpec;
use crate::types::sys::uio::IoVec;
use crate::types::sys::utsname::UtsName;

pub enum SyscallResult {
    Done(c_long),
    Exit(ExitReason),
    Restore(UserContext),
}

pub extern "C" fn handle_syscall(frame: *mut SyscallFrame) -> *mut SyscallFrame {
    // SAFETY: `syscall_handler` passes its register frame.
    let frame = unsafe { &mut *frame };
    match do_handle_syscall(frame) {
        SyscallResult::Done(_) => frame,
        SyscallResult::Exit(reason) => ftl::thread::exit(reason),
        SyscallResult::Restore(ctx) => ctx.restore(),
    }
}

fn do_handle_syscall(frame: &mut SyscallFrame) -> SyscallResult {
    let nr = frame.nr();
    let arg0 = frame.arg0();
    let arg1 = frame.arg1();
    let arg2 = frame.arg2();

    // SAFETY: The reaper still owns a reference to this thread while executing
    //         this handler.
    let system_sp =
        (frame as *const SyscallFrame).addr() + size_of::<SyscallFrame>() + size_of::<FaultFrame>();
    let current = unsafe { Thread::from_system_sp(system_sp) };

    let result = match nr {
        SYS_WRITE => sys_write(current, arg0 as c_int, arg1 as *const c_void, arg2),
        SYS_READ => sys_read(current, arg0 as c_int, arg1 as *mut c_void, arg2),
        SYS_LSEEK => sys_lseek(current, arg0 as c_int, arg1 as off_t, arg2 as c_int),
        SYS_TRUNCATE => sys_truncate(current, arg0 as *const u8, arg1 as off_t),
        SYS_FTRUNCATE => sys_ftruncate(current, arg0 as c_int, arg1 as off_t),
        SYS_FSYNC => sys_fsync(current, arg0 as c_int),
        SYS_FDATASYNC => sys_fsync(current, arg0 as c_int),
        SYS_PREAD64 => {
            sys_pread64(
                current,
                arg0 as c_int,
                arg1 as *mut c_void,
                arg2,
                frame.arg3() as off_t,
            )
        }
        SYS_RECVFROM => {
            sys_recvfrom(
                current,
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
                current,
                arg0 as c_int,
                arg1 as *const c_void,
                arg2,
                frame.arg3() as c_int,
                frame.arg4() as *const u8,
                frame.arg5(),
            )
        }
        SYS_OPEN => sys_open(current, arg0 as *const u8, arg1 as c_int, arg2 as u32),
        SYS_CREAT => sys_creat(current, arg0 as *const u8, arg1 as u32),
        SYS_ACCESS => sys_access(current, arg0 as *const u8, arg1 as c_int),
        SYS_FACCESSAT => sys_faccessat(current, arg0 as c_int, arg1 as *const u8, arg2 as c_int),
        SYS_IOCTL => sys_ioctl(current, arg0 as c_int, arg1 as c_ulong, arg2),
        SYS_OPENAT => {
            sys_openat(
                current,
                arg0 as c_int,
                arg1 as *const u8,
                arg2 as c_int,
                frame.arg3() as u32,
            )
        }
        SYS_MKDIR => sys_mkdir(current, arg0 as *const u8, arg1 as u32),
        SYS_MKDIRAT => sys_mkdirat(current, arg0 as c_int, arg1 as *const u8, arg2 as u32),
        SYS_RMDIR => sys_rmdir(current, arg0 as *const u8),
        SYS_UNLINK => sys_unlink(current, arg0 as *const u8),
        SYS_UNLINKAT => sys_unlinkat(current, arg0 as c_int, arg1 as *const u8, arg2 as c_int),
        SYS_LINK => sys_link(current, arg0 as *const u8, arg1 as *const u8),
        SYS_LINKAT => {
            sys_linkat(
                current,
                arg0 as c_int,
                arg1 as *const u8,
                arg2 as c_int,
                frame.arg3() as *const u8,
                frame.arg4() as c_int,
            )
        }
        SYS_CHMOD => sys_chmod(current, arg0 as *const u8, arg1 as u32),
        SYS_FCHMOD => sys_fchmod(current, arg0 as c_int, arg1 as u32),
        SYS_FCHMODAT => sys_fchmodat(current, arg0 as c_int, arg1 as *const u8, arg2 as u32),
        SYS_UTIMENSAT => {
            sys_utimensat(
                current,
                arg0 as c_int,
                arg1 as *const u8,
                arg2 as *const TimeSpec,
                frame.arg3() as c_int,
            )
        }
        SYS_STAT => sys_stat(current, arg0 as *const u8, arg1 as *mut Stat),
        SYS_FSTAT => sys_fstat(current, arg0 as c_int, arg1 as *mut Stat),
        SYS_LSTAT => sys_lstat(current, arg0 as *const u8, arg1 as *mut Stat),
        SYS_NEWFSTATAT => {
            sys_newfstatat(
                current,
                arg0 as c_int,
                arg1 as *const u8,
                arg2 as *mut Stat,
                frame.arg3() as c_int,
            )
        }
        SYS_STATX => {
            sys_statx(
                current,
                arg0 as c_int,
                arg1 as *const u8,
                arg2 as c_int,
                frame.arg4() as *mut Statx,
            )
        }
        SYS_READLINK => sys_readlink(current, arg0 as *const u8, arg1 as *mut u8, arg2),
        SYS_CLOSE => sys_close(current, arg0 as c_int),
        SYS_CHDIR => sys_chdir(current, arg0 as *const u8),
        SYS_GETCWD => sys_getcwd(current, arg0 as *mut u8, arg1),
        SYS_GETDENTS64 => sys_getdents64(current, arg0 as c_int, arg1 as *mut c_void, arg2),
        SYS_DUP2 => sys_dup2(current, arg0 as c_int, arg1 as c_int),
        SYS_DUP3 => sys_dup3(current, arg0 as c_int, arg1 as c_int, arg2 as c_int),
        SYS_EPOLL_CREATE1 => sys_epoll_create1(current, arg0 as c_int),
        SYS_EPOLL_CTL => {
            sys_epoll_ctl(
                current,
                arg0 as c_int,
                arg1 as c_int,
                arg2 as c_int,
                frame.arg3() as *const EpollEvent,
            )
        }
        SYS_EPOLL_WAIT => {
            sys_epoll_wait(
                current,
                arg0 as c_int,
                arg1 as *mut EpollEvent,
                arg2 as c_int,
                frame.arg3() as c_int,
            )
        }
        SYS_EPOLL_PWAIT => {
            sys_epoll_pwait(
                current,
                arg0 as c_int,
                arg1 as *mut EpollEvent,
                arg2 as c_int,
                frame.arg3() as c_int,
                frame.arg4() as *const c_void,
            )
        }
        SYS_EVENTFD => sys_eventfd(current, arg0 as c_unsigned),
        SYS_EVENTFD2 => sys_eventfd2(current, arg0 as c_unsigned, arg1 as c_int),
        SYS_BRK => sys_brk(current, arg0),
        SYS_MMAP => {
            sys_mmap(
                current,
                arg0,
                arg1,
                arg2 as c_int,
                frame.arg3() as c_int,
                frame.arg4() as c_int,
                frame.arg5() as i64,
            )
        }
        SYS_MPROTECT => sys_mprotect(current, arg0, arg1, arg2 as c_int),
        SYS_MUNMAP => sys_munmap(current, arg0, arg1),
        SYS_MADVISE => sys_madvise(current, arg0, arg1, arg2 as c_int),
        SYS_PIPE => sys_pipe(current, arg0 as *mut c_int),
        SYS_PIPE2 => sys_pipe2(current, arg0 as *mut c_int, arg1 as c_int),
        SYS_POLL => sys_poll(current, arg0 as *mut PollFd, arg1 as nfds_t, arg2 as c_int),
        SYS_PSELECT6 => {
            sys_pselect6(
                current,
                arg0 as c_int,
                arg1 as *mut u8,
                arg2 as *mut u8,
                frame.arg3() as *mut u8,
                frame.arg4() as *const TimeSpec,
            )
        }
        SYS_READV => sys_readv(current, arg0 as c_int, arg1 as *const IoVec, arg2 as c_int),
        SYS_WRITEV => sys_writev(current, arg0 as c_int, arg1 as *const IoVec, arg2 as c_int),
        SYS_CLONE => {
            sys_clone(
                current,
                frame,
                arg0 as c_ulong,
                arg1,
                arg2 as *mut c_int,
                frame.arg3() as *mut c_int,
                frame.arg4() as c_ulong,
            )
        }
        SYS_FORK | SYS_VFORK => sys_fork(current, frame),
        SYS_GETPID => sys_getpid(current),
        SYS_GETPPID => sys_getppid(current),
        SYS_GETTID => sys_gettid(current),
        SYS_GETUID | SYS_GETEUID | SYS_GETGID | SYS_GETEGID => sys_getuid(current),
        SYS_GETGROUPS => sys_getgroups(current, arg0 as c_int),
        SYS_GETPRIORITY => sys_getpriority(current, arg0 as c_int, arg1 as c_int),
        SYS_CAPGET => {
            sys_capget(
                current,
                arg0 as *mut CapUserHeader,
                arg1 as *mut CapUserData,
            )
        }
        SYS_UMASK => sys_umask(current, arg0 as u32),
        SYS_UNAME => sys_uname(current, arg0 as *mut UtsName),
        SYS_SYNC => sys_sync(current),
        SYS_SYSINFO => sys_sysinfo(current, arg0 as *mut SysInfo),
        SYS_FUTEX => {
            sys_futex(
                current,
                arg0 as *mut u32,
                arg1 as c_int,
                arg2 as u32,
                frame.arg3() as *const TimeSpec,
                frame.arg4() as *mut u32,
                frame.arg5() as u32,
            )
        }
        SYS_SCHED_YIELD => sys_sched_yield(current),
        SYS_SCHED_GETAFFINITY => {
            sys_sched_getaffinity(current, arg0 as c_int, arg1, arg2 as *mut c_ulong)
        }
        SYS_GETRANDOM => sys_getrandom(current, arg0 as *mut c_void, arg1, arg2 as c_unsigned),
        SYS_CLOCK_GETTIME => sys_clock_gettime(current, arg0 as c_int, arg1 as *mut TimeSpec),
        SYS_CLOCK_GETRES => sys_clock_getres(current, arg0 as c_int, arg1 as *mut TimeSpec),
        SYS_CLOCK_NANOSLEEP => {
            sys_clock_nanosleep(
                current,
                arg0 as c_int,
                arg1 as c_int,
                arg2 as *const TimeSpec,
                frame.arg3() as *mut TimeSpec,
            )
        }
        SYS_NANOSLEEP => sys_nanosleep(current, arg0 as *const TimeSpec, arg1 as *mut TimeSpec),
        SYS_KILL => sys_kill(current, arg0 as c_int, arg1 as c_int),
        SYS_RT_SIGACTION => {
            sys_rt_sigaction(
                current,
                arg0 as c_int,
                arg1 as *const types::signal::SigAction,
                arg2 as *mut types::signal::SigAction,
                frame.arg3(),
            )
        }
        SYS_RT_SIGRETURN => sys_rt_sigreturn(current, frame),
        SYS_RT_SIGPROCMASK => {
            sys_rt_sigprocmask(
                current,
                arg0 as c_int,
                arg1 as *const u64,
                arg2 as *mut u64,
                frame.arg3(),
            )
        }
        SYS_SIGALTSTACK => {
            sys_sigaltstack(
                current,
                frame,
                arg0 as *const types::signal::StackT,
                arg1 as *mut types::signal::StackT,
            )
        }
        SYS_TGKILL => sys_tgkill(current, arg0 as c_int, arg1 as c_int, arg2 as c_int),
        SYS_SOCKET => sys_socket(current, arg0 as c_int, arg1 as c_int, arg2 as c_int),
        SYS_SHUTDOWN => sys_shutdown(current, arg0 as c_int, arg1 as c_int),
        SYS_BIND => sys_bind(current, arg0 as c_int, arg1 as *const u8, arg2),
        SYS_LISTEN => sys_listen(current, arg0 as c_int, arg1 as c_int),
        SYS_CONNECT => sys_connect(current, arg0 as c_int, arg1 as *const u8, arg2),
        SYS_GETSOCKOPT => {
            sys_getsockopt(
                current,
                arg0 as c_int,
                arg1 as c_int,
                arg2 as c_int,
                frame.arg3() as *mut u8,
                frame.arg4() as *mut u32,
            )
        }
        SYS_GETSOCKNAME => {
            sys_getsockname(current, arg0 as c_int, arg1 as *mut u8, arg2 as *mut u32)
        }
        SYS_GETPEERNAME => {
            sys_getpeername(current, arg0 as c_int, arg1 as *mut u8, arg2 as *mut u32)
        }
        SYS_SETSOCKOPT => {
            sys_setsockopt(
                current,
                arg0 as c_int,
                arg1 as c_int,
                arg2 as c_int,
                frame.arg3() as *const u8,
                frame.arg4(),
            )
        }
        SYS_ACCEPT => sys_accept(current, arg0 as c_int, arg1 as *mut u8, arg2 as *mut u32),
        SYS_ACCEPT4 => {
            sys_accept4(
                current,
                arg0 as c_int,
                arg1 as *mut u8,
                arg2 as *mut u32,
                frame.arg3() as c_int,
            )
        }
        SYS_EXECVE => {
            sys_execve(
                current,
                arg0 as *const u8,
                arg1 as *const *const u8,
                arg2 as *const *const u8,
            )
        }
        SYS_WAIT4 => {
            sys_wait4(
                current,
                arg0 as c_int,
                arg1 as *mut c_int,
                arg2 as c_int,
                frame.arg3() as *mut RUsage,
            )
        }
        SYS_FCNTL => sys_fcntl(current, arg0 as c_int, arg1 as c_int, arg2 as c_long),
        SYS_PRCTL => sys_prctl(current, arg0 as c_int, arg1),
        SYS_ARCH_PRCTL => sys_arch_prctl(current, arg0 as c_int, arg1),
        SYS_SET_TID_ADDRESS => sys_set_tid_address(current, arg0 as *mut c_int),
        SYS_SET_ROBUST_LIST => sys_set_robust_list(current, arg0 as *const c_void, arg1),
        SYS_PRLIMIT64 => {
            sys_prlimit64(
                current,
                arg0 as c_int,
                arg1 as u32,
                arg2 as *const RLimit,
                frame.arg3() as *mut RLimit,
            )
        }
        SYS_EXIT => sys_exit(current, arg0 as c_int),
        SYS_EXIT_GROUP => sys_exit_group(current, arg0 as c_int),
        _ => Err(Errno::ENOSYS),
    };

    let retval = match result {
        Ok(SyscallResult::Done(retval)) => retval,
        Ok(SyscallResult::Exit(reason)) => {
            return SyscallResult::Exit(reason);
        }
        Ok(SyscallResult::Restore(mut ctx)) => {
            current.handle_pending_signal(&mut ctx);
            return SyscallResult::Restore(ctx);
        }
        Err(errno) => -(errno.as_int() as c_long),
    };

    frame.set_retval(retval);

    if current.has_pending_signal() {
        let mut ctx = UserContext::from_syscall_frame(frame);
        current.handle_pending_signal(&mut ctx);
        return SyscallResult::Restore(ctx);
    }

    SyscallResult::Done(retval)
}
