use core::fmt;

/// A pretty printer for byte sizes.
///
/// # Example
///
/// ```
/// use ftl_utils::formatter::ByteSize;
///
/// assert_eq!(format!("{}", ByteSize(128)), "128 B");
/// assert_eq!(format!("{}", ByteSize(1024)), "1 KiB");
/// assert_eq!(format!("{}", ByteSize(16 * 1024 * 1024)), "16 MiB");
/// ```
#[repr(transparent)]
pub struct ByteSize(pub usize);

impl ByteSize {
    #[allow(non_upper_case_globals)]
    pub const KiB: usize = 1024;

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let units = &["B", "KiB", "MiB", "GiB", "TiB"];
        let mut value = self.0;
        let mut i = 0;
        let mut unit = units[0];
        while value >= 1024 && i + 1 < units.len() {
            value /= 1024;
            unit = units[i + 1];
            i += 1;
        }

        write!(f, "{value} {unit}")
    }
}

impl fmt::Debug for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt(f)
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt(f)
    }
}
