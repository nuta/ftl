use alloc::vec::Vec;

use crate::arch;
use crate::cpuvar::current_thread;
use crate::ref_counted::SharedRef;
use crate::spinlock::SpinLock;
use crate::spinlock::SpinLockGuard;
use crate::thread::Thread;

pub struct WaitQueue {
    queue: SpinLock<Vec<SharedRef<Thread>>>,
}

impl WaitQueue {
    pub const fn new() -> WaitQueue {
        WaitQueue {
            queue: SpinLock::new(Vec::new()),
        }
    }

    pub fn listen(&self) {
        let thread = current_thread();
        self.queue.lock().push(thread);
    }

    pub fn wake_all(&self) {
        for waiter in self.queue.lock().drain(..) {
            waiter.set_runnable();
        }
    }
}
