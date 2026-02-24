#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fd(i32);

impl Fd {
    pub const fn from_usize(raw: usize) -> Self {
        Self(raw as i32)
    }
}
