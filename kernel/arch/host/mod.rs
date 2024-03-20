use ftl_types::address::PAddr;

use crate::vm::KVAddr;
use crate::vm::KVAddrArchExt;

pub const PAGE_SIZE: usize = 4096;

pub fn init(_cpu_id: usize) {
    todo!();
}

pub fn idle() {
    todo!()
}

pub fn hang() -> ! {
    todo!()
}

pub fn console_write(_bytes: &[u8]) {
    todo!()
}

pub struct Context {}

impl KVAddrArchExt for KVAddr {
    fn paddr(&self) -> PAddr {
        todo!()
    }
}
