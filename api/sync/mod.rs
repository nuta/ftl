mod mutex;

pub use alloc::sync::Arc;
pub use mutex::{SpinLock, SpinLockGuard};
