use core::arch::asm;

mod start;

use ftl_types::environ::StartInfo;

pub(crate) fn get_start_info() -> &'static StartInfo {
    unsafe {
        let start_info: *const StartInfo;
        asm!("rdgsbase {}", out(reg) start_info);
        &*(start_info as *const StartInfo)
    }
}

pub fn min_page_size() -> usize {
    get_start_info().min_page_size
}

pub fn process_name() -> &'static str {
    let info = get_start_info();
    let ptr = info.name.as_ptr() as *const u8;
    let len = info.name_len as usize;

    // SAFETY: The kernel guarantees that the name is a valid UTF-8 string.
    unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len)) }
}
