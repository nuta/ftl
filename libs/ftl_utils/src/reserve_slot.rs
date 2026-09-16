use alloc::collections::TryReserveError;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::hash::BuildHasher;
use core::hash::Hash;

use hashbrown::HashMap;
use hashbrown::HashSet;

pub trait ReserveSlot {
    type Error;

    fn reserve_slot(&mut self) -> Result<Slot<'_, Self>, Self::Error>;
}

#[must_use]
pub struct Slot<'a, C: ?Sized> {
    this: &'a mut C,
}

impl<T> ReserveSlot for Vec<T> {
    type Error = TryReserveError;

    fn reserve_slot(&mut self) -> Result<Slot<'_, Self>, Self::Error> {
        self.try_reserve(1)?;
        Ok(Slot { this: self })
    }
}

impl<T> Slot<'_, Vec<T>> {
    pub fn push(self, value: T) {
        self.this.push(value);
    }

    pub fn insert(self, index: usize, value: T) {
        self.this.insert(index, value);
    }
}

impl<T> ReserveSlot for VecDeque<T> {
    type Error = TryReserveError;

    fn reserve_slot(&mut self) -> Result<Slot<'_, Self>, Self::Error> {
        self.try_reserve(1)?;
        Ok(Slot { this: self })
    }
}

impl<T> Slot<'_, VecDeque<T>> {
    pub fn push_back(self, value: T) {
        self.this.push_back(value);
    }
}

impl<K, V, S> ReserveSlot for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type Error = hashbrown::TryReserveError;

    fn reserve_slot(&mut self) -> Result<Slot<'_, Self>, Self::Error> {
        self.try_reserve(1)?;
        Ok(Slot { this: self })
    }
}

impl<K, V, S> Slot<'_, HashMap<K, V, S>>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    pub fn insert(self, key: K, value: V) -> Option<V> {
        self.this.insert(key, value)
    }
}

impl<T, S> ReserveSlot for HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Error = hashbrown::TryReserveError;

    fn reserve_slot(&mut self) -> Result<Slot<'_, Self>, Self::Error> {
        self.try_reserve(1)?;
        Ok(Slot { this: self })
    }
}

impl<T, S> Slot<'_, HashSet<T, S>>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    pub fn insert(self, value: T) -> bool {
        self.this.insert(value)
    }
}
