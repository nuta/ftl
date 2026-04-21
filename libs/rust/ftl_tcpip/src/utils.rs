use alloc::collections::VecDeque;

use crate::OutOfMemoryError;

pub trait TryPushBack<T> {
    fn try_push_back(&mut self, value: T) -> Result<(), OutOfMemoryError>;
}

impl<T> TryPushBack<T> for VecDeque<T> {
    fn try_push_back(&mut self, value: T) -> Result<(), OutOfMemoryError> {
        self.try_reserve(1).map_err(|_| OutOfMemoryError)?;
        self.push_back(value);
        Ok(())
    }
}
