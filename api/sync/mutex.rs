pub type SpinLockGuard<'a, T> = spin::MutexGuard<'a, T>;

pub struct SpinLock<T> {
    inner: spin::Mutex<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> SpinLock<T> {
        SpinLock {
            inner: spin::Mutex::new(value),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<T> {
        self.inner.lock()
    }
}
