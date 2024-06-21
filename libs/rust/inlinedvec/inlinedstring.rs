use core::str::FromStr;

use crate::ExceedsCapacityError;
use crate::InlinedVec;

/// An inlined string. Unline `String`, this type is allocated on the stack
/// or inlined in a struct instead of the heap.
///
/// The internal buffer is a `InlinedVec<u8, CAP>`, where `CAP` is the capacity
/// of the string. It's guaranteed to be a valid UTF-8 string.
pub struct InlinedString<const CAP: usize>(InlinedVec<u8, CAP>);

impl<const CAP: usize> InlinedString<CAP> {
    pub fn new() -> Self {
        Self(InlinedVec::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: We guarantee that the string is always a valid UTF-8 string.
        unsafe { core::str::from_utf8_unchecked(self.0.as_slice()) }
    }
}

impl<const CAP: usize> TryFrom<&str> for InlinedString<CAP> {
    type Error = ExceedsCapacityError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let mut string = Self::new();
        string.0.try_extend_from_slice(s.as_bytes())?;
        Ok(string)
    }
}
