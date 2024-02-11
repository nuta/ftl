use alloc::{collections::BTreeMap, ffi::CString, string::ToString};
use arrayvec::ArrayVec;
use ftl_types::{environ::Environ, handle::HandleId, spec::FiberSpec};

use crate::{
    allocator::GLOBAL_ALLOCATOR,
    arch,
    channel::Channel,
    fiber::Fiber,
    print::ByteSize,
    scheduler::{Scheduler, GLOBAL_SCHEDULER},
};

/// A free region of memory available for software.
#[derive(Debug)]
pub struct FreeMem {
    /// The start address of the region.
    pub start: usize,
    /// The size of the region.
    pub size: usize,
}

#[derive(Debug)]
pub struct BootInfo {
    pub free_mems: ArrayVec<FreeMem, 8>,
    pub fiber_inits: &'static [(FiberSpec, fn(*const i8))],
}

pub fn boot(bootinfo: BootInfo) -> ! {
    println!("\nFTL - Faster Than \"L\"\n");

    for entry in bootinfo.free_mems.iter() {
        match *entry {
            FreeMem { start, size } => {
                println!(
                    "free memory: 0x{:016x} - 0x{:016x} ({})",
                    start,
                    start + size,
                    ByteSize::new(size)
                );

                GLOBAL_ALLOCATOR.add_region(start as *mut u8, size);
            }
        }
    }

    arch::init();

    let (ping_ch, pong_ch) = Channel::new().unwrap();
    let mut ping_ch = Some(ping_ch);
    let mut pong_ch = Some(pong_ch);
    for (spec, main) in bootinfo.fiber_inits.iter() {
        let handle = HandleId::new(1);
        let mut deps = BTreeMap::new();
        let ch = if spec.name == "ping" {
            deps.insert("pong".to_string(), handle);
            ping_ch.take().unwrap()
        } else if spec.name == "pong" {
            deps.insert("ping".to_string(), handle);
            pong_ch.take().unwrap()
        } else {
            panic!("unknown fiber: {}", spec.name);
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

    Scheduler::switch_to_next(GLOBAL_SCHEDULER.lock());

    loop {
        arch::idle();
    }
}
