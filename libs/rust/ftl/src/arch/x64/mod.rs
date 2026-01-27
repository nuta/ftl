use core::arch::asm;

mod start;

use ftl_types::environ::StartInfo;

pub fn get_start_info() -> &'static StartInfo {
    unsafe {
        let start_info: *const StartInfo;
        asm!("rdgsbase {}", out(reg) start_info);
        &*(start_info as *const StartInfo)
    }
}
