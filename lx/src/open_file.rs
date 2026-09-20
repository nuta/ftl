use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use core::ops::Deref;

use ftl_utils::spinlock::SpinLock;

use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::off_t;
use crate::types::sys::fcntl::O_NONBLOCK;
use crate::types::sys::socket::MSG_DONTWAIT;
use crate::types::sys::socket::SockAddr;
use crate::types::unistd::SEEK_CUR;
use crate::types::unistd::SEEK_END;
use crate::types::unistd::SEEK_SET;
use crate::vfs::FileLike;

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

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, Errno> {
        let offset = self.mutable.lock().offset;
        let n = self.file.read(buf, offset, self.nonblocking())?;
        self.mutable.lock().offset = offset + n;
        Ok(n)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, Errno> {
        let offset = self.mutable.lock().offset;
        let n = self.file.write(buf, offset, self.nonblocking())?;
        self.mutable.lock().offset = offset + n;
        Ok(n)
    }

    pub fn seek(&self, offset: off_t, whence: c_int) -> Result<off_t, Errno> {
        let offset_usize = offset.try_into().map_err(|_| Errno::EINVAL)?;
        let base = match whence {
            SEEK_SET => 0,
            SEEK_CUR => self.mutable.lock().offset,
            SEEK_END => self.file.size()?,
            _ => return Err(Errno::EINVAL),
        };

        let new_offset = base.checked_add(offset_usize).ok_or(Errno::EINVAL)?;
        self.mutable.lock().offset = new_offset;
        // TODO: Is it possible to guarantee it is in [0, i64::MAX] in a type-safe way?
        let new_offset_i64 = new_offset.try_into().map_err(|_| Errno::EINVAL)?;
        Ok(new_offset_i64)
    }

    pub fn recvfrom(&self, buf: &mut [u8], flags: c_int) -> Result<(usize, SockAddr), Errno> {
        let nonblocking = self.nonblocking() || flags & MSG_DONTWAIT != 0;
        self.file.recvfrom(buf, flags, nonblocking)
    }

    pub fn sendto(&self, buf: &[u8], dest: Option<SockAddr>, flags: c_int) -> Result<usize, Errno> {
        let nonblocking = self.nonblocking() || flags & MSG_DONTWAIT != 0;
        self.file.sendto(buf, dest, flags, nonblocking)
    }

    pub fn accept(&self) -> Result<Arc<dyn FileLike>, Errno> {
        self.file.accept(self.nonblocking())
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
