use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;

use ftl::time::MonoTime;
use ftl::time::MonoTimeExt;
use ftl_types::time::Duration;
use ftl_utils::fxhash::FxHashMap;
use ftl_utils::spinlock::SpinLock;

use crate::open_file::CloseListener;
use crate::open_file::OpenFile;
use crate::types::c_int;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::epoll::EPOLLET;
use crate::types::sys::epoll::EPOLLONESHOT;
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
        let revents = (latest as u32) & event.events;
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
    fn notify(&self, events: Option<c_short>) {
        if let Some(events) = events {
            let interests = self.event.lock().events;
            if (events as u32) & interests == 0 {
                return;
            }
        }

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
    ready_fds: VecDeque<c_int>,
}

struct Inner {
    mutable: SpinLock<Mutable>,
    wait_queue: WaitQueue,
}

impl Inner {
    /// Marks the given file descriptor as ready.
    fn mark_ready(&self, fd: c_int) {
        let mut mutable = self.mutable.lock();
        if !mutable.ready_fds.contains(&fd) {
            mutable.ready_fds.push_back(fd);
        }
        drop(mutable);

        self.wait_queue.notify(None);
    }

    fn detach(&self, fd: c_int) {
        self.mutable.lock().watches.remove(&fd);
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
                let Some(fd) = mutable.ready_fds.pop_front() else {
                    break;
                };

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
            let mut watched = watch.event.lock();
            if watched.events & EPOLLONESHOT != 0 {
                watched.events = 0;
            } else if watched.events & EPOLLET == 0 {
                still_ready_fds.push(fd);
            }
            drop(watched);

            events[n] = event;
            n += 1;
        }

        let mut mutable = self.mutable.lock();
        for fd in still_ready_fds {
            if !mutable.ready_fds.contains(&fd) {
                mutable.ready_fds.push_back(fd);
            }
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
                    ready_fds: VecDeque::new(),
                }),
                wait_queue: WaitQueue::new()?,
            }),
        })
    }

    pub fn add(&self, fd: c_int, mut event: EpollEvent, file: Arc<OpenFile>) -> Result<(), Errno> {
        event.events |= POLLERR as u32 | POLLHUP as u32;
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

    pub fn modify(&self, fd: c_int, mut event: EpollEvent) -> Result<(), Errno> {
        event.events |= POLLERR as u32 | POLLHUP as u32;
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
                self.inner.wait_queue.notify(None);

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
