use ftl_arrayvec::ArrayVec;

use crate::boot::BootInfo;

pub fn console_write(_bytes: &[u8]) {}

pub fn main() -> ! {
    crate::boot::boot(BootInfo {
        free_rams: ArrayVec::new(),
        modules: ArrayVec::new(),
    });
}
