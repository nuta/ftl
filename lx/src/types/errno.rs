use super::c_int;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct Errno(c_int);

impl Errno {
    pub const EPERM: Self = Self(1);
    pub const EINVAL: Self = Self(22);
    pub const ENOSYS: Self = Self(38);

    pub const fn as_int(self) -> c_int {
        self.0
    }
}
