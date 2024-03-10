use core::ops::Deref;
use core::ops::DerefMut;

pub use crate::arch;

pub struct Mutex<T> {
    inner: spin::Mutex<T>,
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Mutex<T> {
        Mutex {
            inner: spin::Mutex::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        // This should come before acquiring the lock to ensure that interrupts are disabled.
        // are disabled while the lock is held.
        let intr_state = arch::IntrStateGuard::save_and_disable_interrupts();

        if self.inner.try_lock().is_none() {
            panic!(
                "Mutex::lock: {:x}: deadlock - mutex will never be left locked in single CPU!",
                self as *const _ as usize
            );
        }

        // println!("Mutex::lock: {:x}: locking", self as *const _ as usize);
        // crate::backtrace::backtrace();

        let inner = self.inner.lock();
        MutexGuard { intr_state, inner }
    }

    pub unsafe fn force_unlock(&self) {
        self.inner.force_unlock()
    }
}

pub struct MutexGuard<'a, T> {
    inner: spin::MutexGuard<'a, T>,
    /// # Don't move this field!
    ///
    /// This field must come *after* `inner` to ensure that interrupts are re-enabled *after* unlocking the lock.
    ///
    /// The ordering is documented in the Rust reference:
    ///
    /// > The fields of a struct are dropped in declaration order.
    /// >
    /// > https://doc.rust-lang.org/reference/destructors.html#destructors
    intr_state: arch::IntrStateGuard,
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.inner
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.inner
    }
}
