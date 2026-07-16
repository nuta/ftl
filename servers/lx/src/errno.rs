#[derive(Debug, Clone, Copy)]
pub struct Errno(isize);

impl Errno {
    pub const EBADF: Errno = Errno(9);
    pub const EFAULT: Errno = Errno(14);
    pub const EINVAL: Errno = Errno(22);
    pub const ENOSYS: Errno = Errno(38);

    pub const fn to_retval(self) -> isize {
        -self.0
    }
}
