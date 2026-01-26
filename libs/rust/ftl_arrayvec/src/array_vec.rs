use core::mem::MaybeUninit;
use core::slice;

/// A fixed-size vector.
///
/// Similar to `Vec<T>`, but using a pre-allocated fixed-sized array instead
/// of allocating memory dynamically.
///
/// # Example
///
/// ```
/// use ftl_arrayvec::ArrayVec;
///
/// let mut vec = ArrayVec::<char, 2>::new();
/// vec.try_push('A');
/// vec.try_push('B');
/// assert_eq!(vec.as_slice(), &['A', 'B']);
/// ```
pub struct ArrayVec<T, const N: usize> {
    elems: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> ArrayVec<T, N> {
    pub const fn new() -> Self {
        Self {
            elems: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn as_slice(&self) -> &[T] {
        let ptr = self.elems.as_ptr();

        // SAFETY: Slots up to self.len are initialized, and MaybeUninit<T> is
        // repr(transparent) and it's safe to access as T.
        unsafe { core::slice::from_raw_parts(ptr as *const T, self.len) }
    }

    pub const fn as_slice_mut(&mut self) -> &mut [T] {
        let ptr = self.elems.as_mut_ptr();

        // SAFETY: Slots up to self.len are initialized, and MaybeUninit<T> is
        // repr(transparent) and it's safe to access as T, and moreover &mut self
        // guarantees that there are no other references to the array.
        unsafe { core::slice::from_raw_parts_mut(ptr as *mut T, self.len) }
    }

    pub const fn try_push(&mut self, value: T) -> Result<(), T> {
        if self.len == N {
            return Err(value);
        }

        self.elems[self.len].write(value);
        self.len += 1;
        Ok(())
    }

    pub const fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;

        // SAFETY: self.len != 0 guarantees that the slot is initialized,
        // and since self.len is decremented, the slot won't be read again.
        Some(unsafe { self.elems[self.len].assume_init_read() })
    }

    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    pub fn iter_mut(&mut self) -> slice::IterMut<'_, T> {
        self.as_slice_mut().iter_mut()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a ArrayVec<T, N> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut ArrayVec<T, N> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T, const N: usize> AsRef<[T]> for ArrayVec<T, N> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_array_vec() {
        let mut vec = ArrayVec::<char, 2>::new();
        assert!(vec.is_empty());
        assert_eq!(vec.len(), 0);

        assert_eq!(vec.try_push('A'), Ok(()));
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.as_ref(), &['A']);

        assert_eq!(vec.try_push('B'), Ok(()));
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.as_ref(), &['A', 'B']);

        assert_eq!(vec.try_push('C'), Err('C'));
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.as_ref(), &['A', 'B']);
    }
}
