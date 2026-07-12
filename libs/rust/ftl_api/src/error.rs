use core::fmt;

#[derive(PartialEq, Eq, Clone, Copy, Hash)]
pub struct ErrorCode(i32);

impl ErrorCode {
    pub const OUT_OF_MEMORY: Self = Self::from_name(b" OOM");
    pub const NOT_ALLOWED: Self = Self::from_name(b"NALW");
    pub const ALREADY_EXISTS: Self = Self::from_name(b"EXST");
    pub const INVALID_ARG: Self = Self::from_name(b"INVA");
    pub const INVALID_STATE: Self = Self::from_name(b"INVS");
    pub const INVALID_TYPE: Self = Self::from_name(b"INVT");
    pub const OUT_OF_BOUNDS: Self = Self::from_name(b" OOB");
    pub const UNSUPPORTED: Self = Self::from_name(b"UNSP");

    const fn from_name(name: &'static [u8]) -> Self {
        if name.len() != 4 {
            panic!("the error name must be 4 characters")
        }

        let mut raw = 0;
        let mut i = 0;
        while i < 4 {
            let ch = name[i];
            if !ch.is_ascii() {
                panic!("the error name must be ASCII")
            }

            raw = (raw << 8) | (ch as i32);
            i += 1;
        }

        Self(raw)
    }
}

impl fmt::Debug for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name_bytes = self.0.to_be_bytes();

        // SAFETY: `from_name` ensures that the name is a valid ASCII string.
        let name = unsafe { core::str::from_utf8_unchecked(&name_bytes) };

        write!(f, "{name}")
    }
}
