#![no_std]
#![no_main]

use ftl_api::environ::Environ;
use ftl_api::prelude::*;

#[no_mangle]
pub fn main(mut env: Environ) {
    info!("env: {:#?}", env);
    let ping_server_ch = env.take_channel("dep:ping_server").unwrap();
    info!("ping_server_ch: {:?}", ping_server_ch);
}
