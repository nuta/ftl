#![no_std]
#![no_main]

use ftl_api::prelude::*;
use ftl_api::types::syscall::VsyscallPage;

static BOOTFS_BIN: &[u8] = include_bytes!("../../build/apps/startup.bin");

#[no_mangle]
pub extern "C" fn ftl_app_main(vsyscall: *const VsyscallPage) {
    unsafe {
        ftl_api::init::init_internal(vsyscall);
    }

    info!("starting up...");
    loop {}
}
