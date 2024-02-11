use alloc::{collections::BTreeMap, ffi::CString, string::ToString};
use ftl_types::{environ::Environ, handle::HandleId};

use crate::{boot::BootInfo, channel::Channel, fiber::Fiber};

pub fn start(bootinfo: &BootInfo) {
    let (ping_ch, pong_ch) = Channel::new().unwrap();
    let mut ping_ch = Some(ping_ch);
    let mut pong_ch = Some(pong_ch);
    for (fiber_name, main) in bootinfo.kernel_fibers.iter() {
        let handle = HandleId::new(1);
        let mut deps = BTreeMap::new();
        let ch = if *fiber_name == "ping" {
            deps.insert("pong".to_string(), handle);
            ping_ch.take().unwrap()
        } else if *fiber_name == "pong" {
            deps.insert("ping".to_string(), handle);
            pong_ch.take().unwrap()
        } else {
            panic!("unknown fiber: {}", fiber_name);
        };

        let environ = Environ { deps };
        let environ_json = serde_json::to_string(&environ).unwrap();
        let environ_cstr = CString::new(environ_json).unwrap();

        let mut fiber = Fiber::new();
        fiber.insert_handle(handle, crate::fiber::Object::Channel(ch));
        fiber.spawn_in_kernel(move || {
            main(environ_cstr.as_ptr());
        });
    }
}
