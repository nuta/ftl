#[derive(Debug, PartialEq, Eq)]
#[repr(isize)]
pub enum FtlError {
    TooManyHandles,
}
