use crate::array_vec::ArrayVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityError;

pub struct ArrayString<const N: usize> {
    inner: ArrayVec<u8, N>,
}

impl<const N: usize> ArrayString<N> {
    pub const fn new() -> Self {
        Self {
            inner: ArrayVec::new(),
        }
    }

    pub const fn from_static(string: &'static str) -> Self {
        if string.len() > N {
            panic!("string length exceeds capacity");
        }

        let mut this = Self::new();
        this.inner.extend_from_slice_unchecked(string.as_bytes());
        this
    }

    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    pub const fn as_str(&self) -> &str {
        // SAFETY: `from_static` ensures the entire &str, which is a valid UTF-8
        // string.
        unsafe { core::str::from_utf8_unchecked(self.inner.as_slice()) }
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.inner.as_slice()
    }

    pub const fn push(&mut self, byte: u8) -> Result<(), CapacityError> {
        match self.inner.try_push(byte) {
            Ok(()) => Ok(()),
            Err(_) => Err(CapacityError),
        }
    }

    pub const fn push_str(&mut self, string: &str) -> Result<(), CapacityError> {
        match self.inner.try_extend_from_slice(string.as_bytes()) {
            Ok(()) => Ok(()),
            Err(_) => Err(CapacityError),
        }
    }
}

impl<const N: usize> AsRef<str> for ArrayString<N> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> TryFrom<&str> for ArrayString<N> {
    type Error = CapacityError;

    fn try_from(string: &str) -> Result<Self, Self::Error> {
        let mut this = Self::new();
        this.push_str(string)?;
        Ok(this)
    }
}
