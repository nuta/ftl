use core::fmt::Debug;

/// Our own version of `unwrap` or `unwrap_unchecked`, but with
/// more descriptive names.
pub trait UnwrapExt<T> {
    /// Unwraps into the inner value. Use this when you are not
    /// sure whether the value is set or not.
    ///
    /// If the value is not set or the value is an error, panics.
    fn unwrap_or_panic(self) -> T;
    /// Unwraps into the inner value. Use this when you're sure that
    /// the value is set.
    ///
    /// If the value is not set or the value is an error,
    /// it panics in debug build or UBs in release build.
    unsafe fn unwrap_always(self) -> T;
}

impl<T, E: Debug> UnwrapExt<T> for core::result::Result<T, E> {
    fn unwrap_or_panic(self) -> T {
        self.unwrap()
    }

    unsafe fn unwrap_always(self) -> T {
        if cfg!(debug_assertions) {
            self.unwrap()
        } else {
            self.unwrap_unchecked()
        }
    }
}

impl<T> UnwrapExt<T> for Option<T> {
    fn unwrap_or_panic(self) -> T {
        self.unwrap()
    }

    unsafe fn unwrap_always(self) -> T {
        if cfg!(debug_assertions) {
            self.unwrap()
        } else {
            self.unwrap_unchecked()
        }
    }
}
