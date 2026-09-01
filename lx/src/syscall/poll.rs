use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::process::OpenFile;
use crate::thread::LxThread;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLNVAL;
use crate::types::sys::poll::PollFd;
use crate::types::sys::poll::nfds_t;
use crate::wait_queue::WaitSet;

pub fn sys_poll(
    current: &LxThread,
    fds: *mut PollFd,
    nfds: nfds_t,
    timeout: c_int,
) -> Result<c_long, Errno> {
    if nfds == 0 {
        return Ok(0);
    }

    if fds.is_null() {
        return Err(Errno::EFAULT);
    }

    let fds = unsafe { core::slice::from_raw_parts_mut(fds, nfds as usize) };
    let process = current.process();

    // Open files to be polled.
    let mut files = Vec::new();
    let fd_table = process.fd_table().lock();
    let mut bad_fds: c_long = 0;
    for e in fds.iter_mut() {
        match fd_table.get(e.fd) {
            Ok(file) => files.push(file.clone()),
            Err(_) => {
                e.revents = POLLNVAL;
                bad_fds += 1;
            }
        }
    }
    drop(fd_table);

    if bad_fds > 0 {
        return Ok(bad_fds);
    }

    if timeout == 0 {
        return scan_pollfds(fds, &files);
    }

    // Subscribe to the files' wait queues.
    let mut wait_set = WaitSet::new()?;
    for file in &files {
        if let Some(wait_queue) = file.wait_queue() {
            wait_set.subscribe(wait_queue);
        }
    }

    loop {
        let n = scan_pollfds(fds, &files)?;
        if n > 0 {
            return Ok(n);
        }

        // No ready entries. Wait for events.
        // TODO: timeout support
        wait_set.wait()?;
    }
}

/// Scans entries, updates them, and returns the number of ready entries.
fn scan_pollfds(fds: &mut [PollFd], files: &[Arc<OpenFile>]) -> Result<c_long, Errno> {
    let mut n = 0;
    for (e, file) in fds.iter_mut().zip(files) {
        let events = e.events;

        e.revents = 0;
        if e.fd < 0 || events == 0 {
            continue;
        }

        // Check the file's status.
        let status = file.poll()?;
        e.revents = events & status;
        if e.revents != 0 {
            n += 1;
        }
    }

    Ok(n)
}
