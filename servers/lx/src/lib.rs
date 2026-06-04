#![cfg_attr(target_os = "none", no_std)]

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    ftl_api::foo();
}
