use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use core::ops::Deref;

use ftl_utils::spinlock::SpinLock;

use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::fcntl::O_NONBLOCK;
use crate::vfs::FileLike;

pub trait CloseListener: Send + Sync {
    fn on_close(&self);
}

struct Mutable {
    flags: c_int,
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

    pub fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, Errno> {
        self.file.read(buf, offset, self.nonblocking())
    }

    pub fn write(&self, buf: &[u8], offset: usize) -> Result<usize, Errno> {
        self.file.write(buf, offset, self.nonblocking())
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
