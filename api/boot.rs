use arrayvec::ArrayVec;

use crate::{
    allocator::GLOBAL_ALLOCATOR,
    arch,
    print::ByteSize,
    task::scheduler::{Scheduler, GLOBAL_SCHEDULER},
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

    let mut map = alloc::collections::BTreeMap::new();
    map.insert("hello", 1);
    map.insert("world", 2);
    println!("map = {:?}", map);

    use crate::{
        sync::channel::{Channel, Message},
        task::fiber::Fiber,
    };

    let (mut ch1_tx, mut ch1_rx) = Channel::new().unwrap();
    let (mut ch2_tx, mut ch2_rx) = Channel::new().unwrap();
    Fiber::spawn(move || {
        for i in 0.. {
            ch1_tx.send(Message::Ping(i)).unwrap();
            let msg = ch2_rx.receive().unwrap();
            println!("filber1: received {:?}", msg);
        }
    });

    Fiber::spawn(move || {
        for i in 0.. {
            let msg = ch1_rx.receive().unwrap();
            println!("filber2: received {:?}", msg);
            ch2_tx.send(Message::Pong(i + 100000000)).unwrap();
        }
    });

    // Fiber::spawn(move || {
    //     println!("fiber A: hello");
    //     for i in 0.. {
    //         crate::arch::yield_cpu();
    //         println!("fiber A: {}", i);
    //     }
    // });

    // Fiber::spawn(move || {
    //     println!("fiber B: world");
    //     for i in 0.. {
    //         crate::arch::yield_cpu();
    //         println!("fiber B: {}", i);
    //     }
    // });

    Scheduler::switch_to_next(GLOBAL_SCHEDULER.lock());

    loop {
        arch::idle();
    }
}
