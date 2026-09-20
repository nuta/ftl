use core::mem::MaybeUninit;

pub struct RingBuffer<T, const CAP: usize> {
    buf: [MaybeUninit<T>; CAP],
    start: usize,
    len: usize,
}

impl<T: Copy, const CAP: usize> RingBuffer<T, CAP> {
    pub const fn new() -> Self {
        Self {
            buf: [MaybeUninit::<T>::uninit(); CAP],
            start: 0,
            len: 0,
        }
    }

    pub fn try_push(&mut self, value: T) -> Result<(), T> {
        if self.len == CAP {
            return Err(value);
        }

        let index = (self.start + self.len) % CAP;
        self.buf[index] = MaybeUninit::new(value);
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        let value = self.buf[self.start];
        self.start = (self.start + 1) % CAP;
        self.len -= 1;

        // SAFETY: The slot is initialized in push.
        Some(unsafe { value.assume_init() })
    }

    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of elements in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer() {
        let mut buffer = RingBuffer::<char, 3>::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);

        assert_eq!(buffer.try_push('A'), Ok(()));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.try_push('B'), Ok(()));
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.try_push('C'), Ok(()));
        assert_eq!(buffer.len(), 3);

        assert_eq!(buffer.try_push('D'), Err('D'));
        assert_eq!(buffer.len(), 3);

        assert_eq!(buffer.pop(), Some('A'));
        assert_eq!(buffer.len(), 2);

        assert_eq!(buffer.try_push('E'), Ok(()));
        assert_eq!(buffer.len(), 3);

        assert_eq!(buffer.pop(), Some('B'));
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.pop(), Some('C'));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.pop(), Some('E'));
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.pop(), None);
    }
}
