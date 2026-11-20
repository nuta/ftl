use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use core::cell::OnceCell;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use ftl::poll::Poll;
use ftl::warn;
use ftl_types::error::ErrorCode;
use ftl_types::time::MonoTime;
use ftl_utils::spinlock::SpinLock;

use crate::process::Process;
use crate::thread::Thread;
use crate::types::c_short;

const WAKE_COOKIE: usize = 0;

pub trait WaitListener: Send + Sync {
    fn notify(&self, events: Option<c_short>);
}

struct Inner {
    poll: Poll,
    wait_ones: AtomicUsize,
    wait_sets: SpinLock<Vec<Arc<Poll>>>,
    listeners: SpinLock<Vec<Weak<dyn WaitListener>>>,
}

impl Inner {
    fn new(poll: Poll) -> Self {
        Self {
            poll,
            wait_ones: AtomicUsize::new(0),
            wait_sets: SpinLock::new(Vec::new()),
            listeners: SpinLock::new(Vec::new()),
        }
    }
}

fn poll_wait_until(poll: &Poll, deadline: MonoTime) -> Result<bool, ErrorCode> {
    match poll.wait_until(deadline) {
        Ok(_) => Ok(false),
        Err(ErrorCode::TimedOut) => Ok(true),
        Err(err) => Err(err),
    }
}

pub struct WaitSet<'a> {
    poll: Arc<Poll>,
    queues: Vec<&'a Inner>,
}

impl<'a> WaitSet<'a> {
    pub fn new(current: &Thread) -> Self {
        Self {
            poll: current.sleep_poll().clone(),
            queues: Vec::new(),
        }
    }

    pub fn subscribe(&mut self, wq: &'a WaitQueue) {
        wq.inner.wait_sets.lock().push(self.poll.clone());
        self.queues.push(&wq.inner);
    }

    pub fn wait(&self) -> Result<(), ErrorCode> {
        self.poll.wait()?;
        Ok(())
    }

    pub fn wait_until(&self, deadline: MonoTime) -> Result<bool, ErrorCode> {
        poll_wait_until(&self.poll, deadline)
    }
}

impl Drop for WaitSet<'_> {
    fn drop(&mut self) {
        for inner in &self.queues {
            let mut polls = inner.wait_sets.lock();
            if let Some(index) = polls
                .iter()
                .position(|other| Arc::ptr_eq(other, &self.poll))
            {
                polls.swap_remove(index);
            }
        }
    }
}

pub struct WaitGuard<'a>(&'a Inner);

impl<'a> WaitGuard<'a> {
    /// Returns `Ok(true)` if the deadline was reached, `Ok(false)` if the
    /// poll returned an event.
    pub fn wait_with_deadline(&self, deadline: MonoTime) -> Result<bool, ErrorCode> {
        poll_wait_until(&self.0.poll, deadline)
    }
}

impl<'a> Drop for WaitGuard<'a> {
    fn drop(&mut self) {
        self.0.wait_ones.fetch_sub(1, Ordering::Relaxed);
    }
}

pub struct WaitQueue {
    inner: Inner,
}

impl WaitQueue {
    pub fn new() -> Result<Self, ErrorCode> {
        Ok(Self {
            inner: Inner::new(Poll::create()?),
        })
    }

    pub fn subscribe(&self) -> WaitGuard<'_> {
        self.inner.wait_ones.fetch_add(1, Ordering::Relaxed);
        WaitGuard(&self.inner)
    }

    pub fn add_listener(&self, listener: Weak<dyn WaitListener>) {
        let mut listeners = self.inner.listeners.lock();
        listeners.push(listener);
    }

    /// Wakes up all waiters on this queue.
    ///
    /// `events` is a `POLL*` bitmask of events that occurred. Specify `None`
    /// to wake up all waiters regardless of their interests.
    pub fn notify(&self, events: Option<c_short>) {
        if let Err(error) = self.do_notify(events) {
            // poll_notify may fail on OOM, but callers wouldn't recover
            // gracefully, and would just log and ignore the error.
            warn!("failed to notify wait queue: {:?}", error);
        }
    }

    fn do_notify(&self, events: Option<c_short>) -> Result<(), ErrorCode> {
        for _ in 0..self.inner.wait_ones.load(Ordering::Relaxed) {
            self.inner.poll.notify(WAKE_COOKIE)?;
        }

        // Notify threads that are waiting on this queue.
        for poll in self.inner.wait_sets.lock().iter() {
            poll.notify(WAKE_COOKIE)?;
        }

        // Get the list of listeners that are still alive.
        let listeners = {
            let mut listeners = self.inner.listeners.lock();
            listeners.retain(|listener| listener.strong_count() > 0);
            listeners.clone()
        };

        // Notify the listeners.
        for listener in &listeners {
            if let Some(listener) = listener.upgrade() {
                listener.notify(events);
            }
        }

        Ok(())
    }
}

/// How to implement a blocking operation in system calls.
///
/// # Example
///
/// ```no_run
/// impl FileLike for MyFile {
///     fn read(&self, buf: &mut [u8], sleep: Sleep) -> Result<usize, Errno> {
///         let guard = sleep.guard(&self.wait_queue);
///         loop {
///              // Read the data if available.
///             let mut data = self.data.lock();
///             if !data.is_empty() {
///                 let n = data.read(buf)?;
///                 return Ok(n);
///             }
///
///             // No data to read. Before waiting, check if the thread was
///             // interrupted.
///             if guard.is_interrupted() {
///                 return Err(Errno::EINTR);
///             }
///
///             // Need to sleep. Subscribe to the wait queue.
///             if !guard.is_subscribed() {
///                 guard.subscribe()?;
///
///                 // Check the data again in case it became available while
///                 // we were subscribing to the queue.
///                 continue;
///             }
///
///             // Wait for self.wait_queue, or a signal.
///             guard.wait()?;
///         }
///     }
/// }
/// ```
#[derive(Clone, Copy)]
pub enum Sleep<'a> {
    Uninterruptible,
    Interruptible {
        thread: &'a Thread,
        process: &'a Process,
    },
}

impl<'a> Sleep<'a> {
    pub fn guard<'b>(self, wq: &'b WaitQueue) -> SleepGuard<'b>
    where
        'a: 'b,
    {
        match self {
            Sleep::Uninterruptible => {
                SleepGuard::Uninterruptible {
                    wq,
                    guard: OnceCell::new(),
                }
            }
            Sleep::Interruptible { thread, process } => {
                SleepGuard::Interruptible {
                    wq,
                    thread,
                    process,
                    set: OnceCell::new(),
                }
            }
        }
    }
}

pub enum SleepGuard<'a> {
    Uninterruptible {
        wq: &'a WaitQueue,
        guard: OnceCell<WaitGuard<'a>>,
    },
    Interruptible {
        wq: &'a WaitQueue,
        thread: &'a Thread,
        process: &'a Process,
        set: OnceCell<WaitSet<'a>>,
    },
}

impl SleepGuard<'_> {
    pub fn is_interrupted(&self) -> bool {
        match self {
            SleepGuard::Uninterruptible { .. } => false,
            SleepGuard::Interruptible { thread, .. } => thread.has_pending_signal(),
        }
    }

    pub fn is_subscribed(&self) -> bool {
        match self {
            SleepGuard::Uninterruptible { guard, .. } => guard.get().is_some(),
            SleepGuard::Interruptible { set, .. } => set.get().is_some(),
        }
    }

    pub fn subscribe(&self) -> Result<(), ErrorCode> {
        match self {
            SleepGuard::Uninterruptible { wq, guard } => {
                if guard.set(wq.subscribe()).is_err() {
                    unreachable!();
                }
            }
            SleepGuard::Interruptible {
                wq,
                thread,
                process,
                set,
            } => {
                // Wait for both the wait queue and the signal.
                let mut new_set = WaitSet::new(thread);
                new_set.subscribe(wq);
                new_set.subscribe(process.signal_wait_queue());
                if set.set(new_set).is_err() {
                    unreachable!();
                }
            }
        }

        Ok(())
    }

    fn poll(&self) -> &Poll {
        match self {
            SleepGuard::Uninterruptible { guard, .. } => {
                &guard.get().expect("wait before subscribe").0.poll
            }
            SleepGuard::Interruptible { set, .. } => {
                &set.get().expect("wait before subscribe").poll
            }
        }
    }

    pub fn wait(&self) -> Result<(), ErrorCode> {
        self.poll().wait()?;
        Ok(())
    }

    pub fn wait_until(&self, deadline: MonoTime) -> Result<bool, ErrorCode> {
        poll_wait_until(self.poll(), deadline)
    }
}
