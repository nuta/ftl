use alloc::vec::Vec;

use crate::OutOfMemoryError;

pub(crate) trait VecExt<T> {
    fn try_push(&mut self, value: T) -> Result<(), OutOfMemoryError>;
}

impl<T> VecExt<T> for Vec<T> {
    fn try_push(&mut self, value: T) -> Result<(), OutOfMemoryError> {
        self.try_reserve(1).map_err(|_| OutOfMemoryError)?;
        self.push(value);
        Ok(())
    }
}
