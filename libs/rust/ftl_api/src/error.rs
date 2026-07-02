#[derive(Debug, PartialEq, Eq)]
pub struct ErrorCode(i32);

impl ErrorCode {
    pub const OUT_OF_MEMORY: Self = Self(1);
}
