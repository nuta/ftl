use core::{fmt, ops::{Deref, DerefMut}};

#[derive(Debug, PartialEq, Eq)]
pub struct ExceedsCapacityError;

pub struct InlinedVec<T: Default, const CAP: usize>(tinyvec::ArrayVec<[T; CAP]>);

impl<T: Default, const CAP: usize> InlinedVec<T, CAP> {
    pub fn new() -> Self {
        Self(tinyvec::ArrayVec::new())
    }

    pub fn try_push(&mut self, value: T) -> Result<(), T> {
        match self.0.try_push(value) {
            Some(value) => Err(value),
            None => Ok(()),
        }
    }

    pub fn try_extend_from_slice(&mut self, other: &[T]) -> Result<(), ExceedsCapacityError> where T: Copy  {
        // Since tinyvec doesn't provide a way to "try" extending from a slice,
        // check the length manually to avoid panicking.
        if other.len() > CAP - self.0.len() {
            return Err(ExceedsCapacityError);
        }

        self.0.extend_from_slice(other);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.0.as_mut_slice()
    }
}

impl<T: Default, const CAP: usize> Deref for InlinedVec<T, CAP> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}

impl<T: Default, const CAP: usize> DerefMut for InlinedVec<T, CAP> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut_slice()
    }
}

impl<T: Default, const CAP: usize> AsRef<[T]> for InlinedVec<T, CAP> {
    fn as_ref(&self) -> &[T] {
        self.0.as_slice()
    }
}


impl<T: Default, const CAP: usize> AsMut<[T]> for InlinedVec<T, CAP> {
    fn as_mut(&mut self) -> &mut [T] {
        self.0.as_mut_slice()
    }
}

impl<T: Default, const CAP: usize> fmt::Debug for InlinedVec<T, CAP>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Default, const CAP: usize> Default for InlinedVec<T, CAP> {
    fn default() -> Self {
        Self::new()
    }
}
