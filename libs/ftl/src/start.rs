use core::slice;

use ftl_types::thread::ExitReason;

unsafe extern "Rust" {
    fn main(cmdline: &[u8]);
}

#[cfg(target_os = "none")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn start() {
    core::arch::naked_asm!(
        "mov rdi, [rsp]", // cmdline pointer
        "mov rsi, [rsp + 8]", // cmdline len
        "call {entry}",
        "ud2",
        entry = sym start_main,
    );
}

unsafe extern "C" fn start_main(cmdline_ptr: *const u8, len: usize) {
    // SAFETY: The kernel placed these bytes in the initial thread's stack.
    unsafe {
        let cmdline = slice::from_raw_parts(cmdline_ptr, len);
        main(cmdline);
    }

    crate::thread::exit(ExitReason::Success);
}
