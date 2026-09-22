use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use ftl::poll::Poll;
use ftl_types::error::ErrorCode;
use ftl_types::poll::EventKind;
use ftl_types::time::MonoTime;
use ftl_utils::spinlock::SpinLock;

use crate::process::Process;

pub trait WaitListener: Send + Sync {
    fn notify(&self);
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

pub struct WaitSet<'a> {
    poll: Arc<Poll>,
    queues: Vec<&'a Inner>,
}

impl<'a> WaitSet<'a> {
    pub fn new() -> Result<Self, ErrorCode> {
        Ok(Self {
            poll: Arc::new(Poll::create()?),
            queues: Vec::new(),
        })
    }

    pub fn subscribe(&mut self, wq: &'a WaitQueue) {
        wq.inner.wait_sets.lock().push(self.poll.clone());
        self.queues.push(&wq.inner);
    }

    pub fn wait(&self) -> Result<(), ErrorCode> {
        self.poll.wait()?;
        Ok(())
    }

    pub fn wait_with_deadline(&self, deadline: MonoTime) -> Result<bool, ErrorCode> {
        let ev = self.poll.wait_until(deadline)?;
        Ok(ev.kind() == EventKind::PollTimeout)
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
    pub fn wait(&self) -> Result<(), ErrorCode> {
        self.0.poll.wait()?;
        Ok(())
    }

    /// Returns `Ok(true)` if the deadline was reached, `Ok(false)` if the
    /// poll returned an event.
    pub fn wait_with_deadline(&self, deadline: MonoTime) -> Result<bool, ErrorCode> {
        let ev = self.0.poll.wait_until(deadline)?;
        Ok(ev.kind() == EventKind::PollTimeout)
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

    pub fn notify_all(&self) -> Result<(), ErrorCode> {
        for _ in 0..self.inner.wait_ones.load(Ordering::Relaxed) {
            self.inner.poll.notify()?;
        }

        // Notify threads that are waiting on this queue.
        for poll in self.inner.wait_sets.lock().iter() {
            poll.notify()?;
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
                listener.notify();
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
///         let mut guard = sleep.guard(&self.wait_queue)?;
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
///             if guard.interrupted() {
///                 return Err(Errno::EINTR);
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
    Interruptible(&'a Process),
}

pub enum SleepGuard<'a> {
    Uninterruptible(WaitGuard<'a>),
    Interruptible {
        set: WaitSet<'a>,
        process: &'a Process,
    },
}

impl<'a> Sleep<'a> {
    pub fn guard<'b>(self, wq: &'b WaitQueue) -> Result<SleepGuard<'b>, ErrorCode>
    where
        'a: 'b,
    {
        match self {
            Sleep::Uninterruptible => Ok(SleepGuard::Uninterruptible(wq.subscribe())),
            Sleep::Interruptible(process) => {
                // Wait for both the wait queue and the signal.
                let mut set = WaitSet::new()?;
                set.subscribe(wq);
                set.subscribe(process.signal_wait_queue());
                Ok(SleepGuard::Interruptible { set, process })
            }
        }
    }
}

impl SleepGuard<'_> {
    pub fn is_interrupted(&self) -> bool {
        match self {
            SleepGuard::Uninterruptible(_) => false,
            SleepGuard::Interruptible { process, .. } => process.has_pending_signal(),
        }
    }

    pub fn wait(&self) -> Result<(), ErrorCode> {
        match self {
            SleepGuard::Uninterruptible(guard) => guard.wait(),
            SleepGuard::Interruptible { set, .. } => set.wait(),
        }
    }

    pub fn wait_until(&self, deadline: MonoTime) -> Result<bool, ErrorCode> {
        match self {
            SleepGuard::Uninterruptible(guard) => guard.wait_with_deadline(deadline),
            SleepGuard::Interruptible { set, .. } => set.wait_with_deadline(deadline),
        }
    }
}
