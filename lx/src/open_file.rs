use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use core::mem::offset_of;
use core::ops::Deref;

use ftl_utils::alignment::align_up;
use ftl_utils::spinlock::SpinLock;

use crate::signal::Signal;
use crate::types::c_int;
use crate::types::dirent::DT_DIR;
use crate::types::dirent::DT_REG;
use crate::types::dirent::Dirent64;
use crate::types::errno::Errno;
use crate::types::off_t;
use crate::types::sys::fcntl::O_NONBLOCK;
use crate::types::sys::socket::MSG_DONTWAIT;
use crate::types::sys::socket::SockAddr;
use crate::types::unistd::SEEK_CUR;
use crate::types::unistd::SEEK_END;
use crate::types::unistd::SEEK_SET;
use crate::vfs::FileLike;
use crate::vfs::INode;
use crate::vfs::IoVecSlice;
use crate::wait_queue::Sleep;

pub trait CloseListener: Send + Sync {
    fn on_close(&self);
}

struct Mutable {
    flags: c_int,
    offset: usize,
    close_listeners: Vec<Weak<dyn CloseListener>>,
}

/// An opened file.
///
/// This is a simple wrapper that ensures that the underlying file is closed
/// when all `Arc<OpenFile>` are dropped.
pub struct OpenFile {
    file: Arc<dyn FileLike>,
    mutable: SpinLock<Mutable>,
}

impl OpenFile {
    pub(crate) fn new(file: Arc<dyn FileLike>, flags: c_int) -> Self {
        Self {
            file,
            mutable: SpinLock::new(Mutable {
                flags,
                offset: 0,
                close_listeners: Vec::new(),
            }),
        }
    }

    pub fn add_close_listener(&self, listener: Weak<dyn CloseListener>) {
        let mut mutable = self.mutable.lock();

        // Garbage collect dropped listeners first.
        mutable
            .close_listeners
            .retain(|listener| listener.strong_count() > 0);

        mutable.close_listeners.push(listener);
    }

    pub fn flags(&self) -> c_int {
        self.mutable.lock().flags
    }

    pub fn set_status_flags(&self, flags: c_int) -> Result<(), Errno> {
        let mut mutable = self.mutable.lock();
        if flags & O_NONBLOCK != 0 {
            mutable.flags |= O_NONBLOCK;
        } else {
            mutable.flags &= !O_NONBLOCK;
        }

        Ok(())
    }

    fn nonblocking(&self) -> bool {
        self.flags() & O_NONBLOCK != 0
    }

    pub fn read(&self, buf: &mut [u8], sleep: Sleep<'_>) -> Result<usize, Errno> {
        let offset = self.mutable.lock().offset;
        let n = self.file.read(buf, offset, self.nonblocking(), sleep)?;
        self.mutable.lock().offset = offset + n;
        Ok(n)
    }

    pub fn do_write(&self, buf: &[u8], sleep: Sleep<'_>) -> Result<usize, Errno> {
        let offset = self.mutable.lock().offset;
        let n = self.file.write(buf, offset, self.nonblocking(), sleep)?;
        self.mutable.lock().offset = offset + n;
        Ok(n)
    }

    pub fn do_writev(&self, iovecs: &IoVecSlice, sleep: Sleep<'_>) -> Result<usize, Errno> {
        let offset = self.mutable.lock().offset;
        let n = self
            .file
            .writev(iovecs, offset, self.nonblocking(), sleep)?;
        self.mutable.lock().offset = offset + n;
        Ok(n)
    }

    pub fn write(&self, buf: &[u8], sleep: Sleep<'_>) -> Result<usize, Errno> {
        match self.do_write(buf, sleep) {
            Ok(n) => Ok(n),
            Err(Errno::EPIPE) => {
                // On EPIPE, trigger PIPE signal before returning the error.
                if let Sleep::Interruptible(process) = sleep {
                    let _ = process.queue_signal(Signal::PIPE);
                }
                Err(Errno::EPIPE)
            }
            Err(error) => Err(error),
        }
    }

    pub fn writev(&self, iovecs: &IoVecSlice, sleep: Sleep<'_>) -> Result<usize, Errno> {
        match self.do_writev(iovecs, sleep) {
            Ok(n) => Ok(n),
            Err(Errno::EPIPE) => {
                // On EPIPE, trigger PIPE signal before returning the error.
                if let Sleep::Interruptible(process) = sleep {
                    let _ = process.queue_signal(Signal::PIPE);
                }
                Err(Errno::EPIPE)
            }
            Err(error) => Err(error),
        }
    }

    pub fn seek(&self, offset: off_t, whence: c_int) -> Result<off_t, Errno> {
        let offset_isize = offset.try_into().map_err(|_| Errno::EINVAL)?;
        let base = match whence {
            SEEK_SET => 0,
            SEEK_CUR => self.mutable.lock().offset,
            SEEK_END => self.file.size()?,
            _ => return Err(Errno::EINVAL),
        };

        let new_offset = base.checked_add_signed(offset_isize).ok_or(Errno::EINVAL)?;
        self.mutable.lock().offset = new_offset;
        // TODO: Is it possible to guarantee it is in [0, i64::MAX] in a type-safe way?
        let new_offset_i64 = new_offset.try_into().map_err(|_| Errno::EINVAL)?;
        Ok(new_offset_i64)
    }

    /// Writes directory entries (`struct dirent64`) to `buf`.
    pub fn getdents(&self, buf: &mut [u8]) -> Result<usize, Errno> {
        let mut mutable = self.mutable.lock();
        let mut written = 0;
        while let Some(entry) = self.file.readdir(mutable.offset)? {
            let name_offset = offset_of!(Dirent64, d_name);

            // Calculate the length of this entry.
            let reclen = align_up(name_offset + entry.name.len() + 1, align_of::<Dirent64>());

            // Do a range check, and get the slice for this entry.
            let Some(dirent) = buf.get_mut(written..written + reclen) else {
                break;
            };

            let header = Dirent64 {
                d_ino: mutable.offset as u64 + 1,
                d_off: mutable.offset as i64 + 1,
                d_reclen: reclen as u16,
                d_type: match entry.inode {
                    INode::Dir(_) => DT_DIR,
                    INode::File(_) => DT_REG,
                },
                d_name: [],
            };

            // Write the header.
            unsafe {
                dirent
                    .as_mut_ptr()
                    .cast::<Dirent64>()
                    .write_unaligned(header)
            };

            // Write the name string, which is next to the header.
            dirent[name_offset..][..entry.name.len()].copy_from_slice(entry.name);
            // Write the null terminator.
            dirent[name_offset + entry.name.len()] = 0;

            written += reclen;
            mutable.offset += 1;
        }

        Ok(written)
    }

    pub fn recvfrom(
        &self,
        buf: &mut [u8],
        flags: c_int,
        sleep: Sleep<'_>,
    ) -> Result<(usize, SockAddr), Errno> {
        let nonblocking = self.nonblocking() || flags & MSG_DONTWAIT != 0;
        self.file.recvfrom(buf, flags, nonblocking, sleep)
    }

    pub fn sendto(
        &self,
        buf: &[u8],
        dest: Option<SockAddr>,
        flags: c_int,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let nonblocking = self.nonblocking() || flags & MSG_DONTWAIT != 0;
        self.file.sendto(buf, dest, flags, nonblocking, sleep)
    }

    pub fn accept(&self, sleep: Sleep<'_>) -> Result<Arc<dyn FileLike>, Errno> {
        self.file.accept(self.nonblocking(), sleep)
    }
}

impl Deref for OpenFile {
    type Target = dyn FileLike;

    fn deref(&self) -> &Self::Target {
        self.file.as_ref()
    }
}

impl Drop for OpenFile {
    fn drop(&mut self) {
        let mut mutable = self.mutable.lock();

        // Notify all listeners.
        let listeners = core::mem::take(&mut mutable.close_listeners);
        for listener in listeners {
            if let Some(listener) = listener.upgrade() {
                listener.on_close();
            }
        }

        self.file.close();
    }
}
