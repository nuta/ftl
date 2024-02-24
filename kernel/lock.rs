pub struct Mutex<T> {
    inner: spin::Mutex<T>,
}

pub type MutexGuard<'a, T> = spin::MutexGuard<'a, T>;

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Mutex<T> {
        Mutex {
            inner: spin::Mutex::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        if self.inner.try_lock().is_none() {
            panic!("Mutex::lock: {:x}: deadlock", self as *const _ as usize);
        }

        // println!("Mutex::lock: {:x}: locking", self as *const _ as usize);
        // crate::backtrace::backtrace();

        self.inner.lock()
    }

    pub unsafe fn force_unlock(&self) {
        self.inner.force_unlock()
    }
}
