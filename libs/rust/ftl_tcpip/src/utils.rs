use alloc::vec::Vec;
use core::hash::Hash;

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

pub(crate) trait HashMapExt<K, V> {
    fn reserve_and_insert(&mut self, key: K, value: V) -> Result<Option<V>, OutOfMemoryError>;
}

impl<K: Eq + Hash, V> HashMapExt<K, V> for hashbrown::HashMap<K, V> {
    fn reserve_and_insert(&mut self, key: K, value: V) -> Result<Option<V>, OutOfMemoryError> {
        self.try_reserve(1).map_err(|_| OutOfMemoryError)?;
        Ok(self.insert(key, value))
    }
}
