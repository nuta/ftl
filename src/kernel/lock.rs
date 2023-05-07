use core::{cell::UnsafeCell, ops::Deref};

use crate::arch;

pub struct GiantLock<T> {
    inner: UnsafeCell<T>,
}

impl<T> GiantLock<T> {
    pub const fn new(inner: T) -> Self {
        Self {
            inner: UnsafeCell::new(inner),
        }
    }

    pub fn get(&self) -> &T {
        debug_assert!(arch::owns_giant_lock());
        unsafe { &*self.inner.get() }
    }

    pub fn get_mut(&self) -> &mut T {
        debug_assert!(arch::owns_giant_lock());
        unsafe { &mut *self.inner.get() }
    }
}

impl<T> Deref for GiantLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

unsafe impl<T> Sync for GiantLock<T> {}
