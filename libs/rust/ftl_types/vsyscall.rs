use crate::error::FtlError;

pub struct VsyscallPage {
    pub entry: fn(isize, isize, isize, isize, isize, isize) -> Result<isize, FtlError>,
}
