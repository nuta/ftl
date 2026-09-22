use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;

use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl::trace;
use ftl_types::time::Duration;
use ftl_utils::fxhash::FxHashMap;
use ftl_utils::fxhash::FxHashSet;
use ftl_utils::spinlock::SpinLock;

use crate::open_file::CloseListener;
use crate::open_file::OpenFile;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::sys::epoll::EPOLLET;
use crate::types::sys::epoll::EpollEvent;
use crate::types::sys::poll::POLLERR;
use crate::types::sys::poll::POLLHUP;
use crate::vfs::FileLike;
use crate::wait_queue::Sleep;
use crate::wait_queue::WaitListener;
use crate::wait_queue::WaitQueue;

struct Watch {
    fd: c_int,
    file: Weak<OpenFile>,
    event: SpinLock<EpollEvent>,
    inner: Weak<Inner>,
}

impl Watch {
    fn poll(&self) -> Result<Option<EpollEvent>, Errno> {
        let Some(file) = self.file.upgrade() else {
            // The file has been closed. Remove this watch.
            self.detach();
            return Ok(None);
        };

        let event = *self.event.lock();
        let latest = file.poll()?;
        // If POLLERR/POLLHUP is set, report regardless of the events mask.
        let revents = (latest as u32) & (event.events | POLLERR as u32 | POLLHUP as u32);
        if revents == 0 {
            return Ok(None);
        }

        Ok(Some(EpollEvent {
            events: revents,
            data: event.data,
        }))
    }

    fn detach(&self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.detach(self.fd);
        }
    }
}

impl WaitListener for Watch {
    fn notify(&self) {
        if let Some(epoll) = self.inner.upgrade() {
            epoll.mark_ready(self.fd);
        }
    }
}

impl CloseListener for Watch {
    fn on_close(&self) {
        self.detach();
    }
}

struct Mutable {
    watches: FxHashMap<c_int, Arc<Watch>>,
    ready_fds: FxHashSet<c_int>,
}

struct Inner {
    mutable: SpinLock<Mutable>,
    wait_queue: WaitQueue,
}

impl Inner {
    /// Marks the given file descriptor as ready.
    fn mark_ready(&self, fd: c_int) {
        self.mutable.lock().ready_fds.insert(fd);
        let _ = self.wait_queue.notify_all();
    }

    fn detach(&self, fd: c_int) {
        let mut mutable = self.mutable.lock();
        mutable.watches.remove(&fd);
        mutable.ready_fds.remove(&fd);
    }

    /// Polls the watch and marks it ready only if it matches the interest mask.
    fn mark_changed(&self, watch: &Watch) -> Result<(), Errno> {
        let events = watch.poll()?;
        if events.is_some() {
            self.mark_ready(watch.fd);
        }

        Ok(())
    }

    fn drain(&self, events: &mut [EpollEvent]) -> Result<usize, Errno> {
        let mut n = 0;
        let mut still_ready_fds = Vec::new();
        while n < events.len() {
            // Pop a fd from the ready list.
            let (fd, watch) = {
                let mut mutable = self.mutable.lock();
                let Some(fd) = mutable.ready_fds.iter().next().copied() else {
                    break;
                };

                mutable.ready_fds.remove(&fd);
                (fd, mutable.watches.get(&fd).cloned())
            };

            let Some(watch) = watch else {
                // The file has notifies us, but we no longer watch it anymore.
                continue;
            };

            let Ok(event) = watch.poll() else {
                // It failed to check the readiness of the file.
                continue;
            };

            let Some(event) = event else {
                // No interesting events.
                continue;
            };

            // In edge-triggered mode (EPOLLET), skip checking the same fd
            // again.
            if watch.event.lock().events & EPOLLET == 0 {
                still_ready_fds.push(fd);
            }

            events[n] = event;
            n += 1;
        }

        if !still_ready_fds.is_empty() {
            self.mutable.lock().ready_fds.extend(still_ready_fds);
        }

        Ok(n)
    }
}

pub struct Epoll {
    inner: Arc<Inner>,
}

impl Epoll {
    pub fn new() -> Result<Self, Errno> {
        Ok(Self {
            inner: Arc::new(Inner {
                mutable: SpinLock::new(Mutable {
                    watches: FxHashMap::new(),
                    ready_fds: FxHashSet::new(),
                }),
                wait_queue: WaitQueue::new()?,
            }),
        })
    }

    pub fn add(&self, fd: c_int, event: EpollEvent, file: Arc<OpenFile>) -> Result<(), Errno> {
        let watch = Arc::new(Watch {
            fd,
            file: Arc::downgrade(&file),
            event: SpinLock::new(event),
            inner: Arc::downgrade(&self.inner),
        });

        {
            let mut mutable = self.inner.mutable.lock();
            if mutable.watches.contains_key(&fd) {
                return Err(Errno::EEXIST);
            }

            mutable.watches.insert(fd, watch.clone());
        }

        // Notify the watch when the file is closed.
        file.add_close_listener(Arc::downgrade(&watch) as Weak<dyn CloseListener>);

        // Register this to the target file.
        if let Some(wait_queue) = file.wait_queue() {
            let listener = Arc::downgrade(&watch);
            wait_queue.add_listener(listener);
        }

        self.inner.mark_changed(&watch)?;
        Ok(())
    }

    pub fn modify(&self, fd: c_int, event: EpollEvent) -> Result<(), Errno> {
        let watch = {
            let mutable = self.inner.mutable.lock();
            mutable.watches.get(&fd).cloned()
        };

        let watch = watch.ok_or(Errno::ENOENT)?;
        *watch.event.lock() = event;

        self.inner.mark_changed(&watch)?;
        Ok(())
    }

    pub fn delete(&self, fd: c_int) -> Result<(), Errno> {
        let mut mutable = self.inner.mutable.lock();
        mutable.watches.remove(&fd).ok_or(Errno::ENOENT)?;
        mutable.ready_fds.remove(&fd);
        Ok(())
    }

    pub fn wait(
        &self,
        events: &mut [EpollEvent],
        timeout: c_int,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let deadline = if let Ok(timeout) = timeout.try_into() {
            let duration = Duration::from_millis(timeout);
            Some(MonoTime::now() + duration)
        } else {
            None
        };

        let sleep_guard = sleep.guard(&self.inner.wait_queue)?;
        loop {
            let n = self.inner.drain(events)?;
            if n > 0 {
                // Found some ready events.

                // Unsubscribe first so that notify_all below won't notify ourself.
                drop(sleep_guard);

                // Concurrent readers may want to read the fds that we've
                // temporarily popped. Notify them to re-check the readiness.
                if let Err(e) = self.inner.wait_queue.notify_all() {
                    trace!("Failed to notify wait queue: {:?}", e);
                }

                return Ok(n);
            }

            if timeout == 0 {
                return Ok(0);
            }

            if sleep_guard.is_interrupted() {
                return Err(Errno::EINTR);
            }

            if let Some(deadline) = deadline {
                if sleep_guard.wait_until(deadline)? {
                    // The deadline was reached.
                    return Ok(0);
                }
            } else {
                sleep_guard.wait()?;
            }
        }
    }
}

impl FileLike for Epoll {
    fn as_epoll(&self) -> Option<&Epoll> {
        Some(self)
    }
}
