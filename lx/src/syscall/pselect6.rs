use alloc::vec::Vec;
use core::cmp::min;
use core::slice;

use super::SyscallResult;
use super::poll::sys_poll;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLERR;
use crate::types::sys::poll::POLLHUP;
use crate::types::sys::poll::POLLIN;
use crate::types::sys::poll::POLLNVAL;
use crate::types::sys::poll::POLLOUT;
use crate::types::sys::poll::PollFd;
use crate::types::sys::poll::nfds_t;
use crate::types::sys::time::TimeSpec;

const FD_SETSIZE: usize = 1024;

struct FdSet<'a> {
    bits: &'a mut [u8],
}

impl FdSet<'_> {
    fn from_user(set: *mut u8, nfds: usize) -> Self {
        if set.is_null() {
            return Self { bits: &mut [] };
        }

        let bits = unsafe { slice::from_raw_parts_mut(set, nfds.div_ceil(8)) };
        Self { bits }
    }

    fn contains(&self, fd: usize) -> bool {
        match self.bits.get(fd / 8) {
            Some(byte) => byte & (1 << (fd % 8)) != 0,
            None => false,
        }
    }

    fn insert(&mut self, fd: usize) {
        self.bits[fd / 8] |= 1 << (fd % 8);
    }

    fn clear(&mut self) {
        self.bits.fill(0);
    }
}

pub fn sys_pselect6(
    current: &LxThread,
    nfds: c_int,
    readfds: *mut u8,
    writefds: *mut u8,
    exceptfds: *mut u8,
    timeout: *const TimeSpec,
) -> Result<SyscallResult, Errno> {
    let Ok(nfds) = usize::try_from(nfds) else {
        return Err(Errno::EINVAL);
    };

    if nfds > FD_SETSIZE {
        return Err(Errno::EINVAL);
    }

    // Parse the timeout.
    let timeout_ms = if timeout.is_null() {
        // Wait indefinitely.
        -1
    } else {
        let timespec = unsafe { *timeout };
        let Some(nanos) = timespec.to_nanos() else {
            return Err(Errno::EINVAL);
        };

        // Round up to milliseconds, as defined in pselect(2).
        min(nanos.div_ceil(1_000_000), c_int::MAX as u64) as c_int
    };

    let mut readfds = FdSet::from_user(readfds, nfds);
    let mut writefds = FdSet::from_user(writefds, nfds);
    let mut exceptfds = FdSet::from_user(exceptfds, nfds);

    // Convert the sets into Vec<PollFd>.
    let mut fds = Vec::new();
    for fd in 0..nfds {
        let mut events = 0;
        if readfds.contains(fd) {
            events |= POLLIN;
        }

        if writefds.contains(fd) {
            events |= POLLOUT;
        }

        if events != 0 {
            fds.push(PollFd {
                fd: fd as c_int,
                events,
                revents: 0,
            });
        }
    }

    // TODO: Support sigmask.
    sys_poll(current, fds.as_mut_ptr(), fds.len() as nfds_t, timeout_ms)?;

    // Clear the fd sets before writing file descriptors.
    readfds.clear();
    writefds.clear();
    exceptfds.clear();

    // Write back ready file descriptors.
    let mut n: c_long = 0;
    for e in &fds {
        if e.revents & POLLNVAL != 0 {
            return Err(Errno::EBADF);
        }

        let fd = e.fd as usize;

        if e.events & POLLIN != 0 && e.revents & (POLLIN | POLLHUP | POLLERR) != 0 {
            readfds.insert(fd);
            n += 1;
        }

        if e.events & POLLOUT != 0 && e.revents & (POLLOUT | POLLERR) != 0 {
            writefds.insert(fd);
            n += 1;
        }
    }

    Ok(SyscallResult::Done(n))
}
