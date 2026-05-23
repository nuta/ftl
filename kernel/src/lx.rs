use alloc::collections::btree_map::BTreeMap;

use ftl_utils::alignment::align_up;

use crate::address::UAddr;
use crate::arch;
use crate::arch::PageAttrs;
use crate::shared_ref::SharedRef;
use crate::thread::Thread;
use crate::vmarea::VmArea;
use crate::vmspace::VmSpace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PId(i32);

struct LxProcess {
    id: PId,
    thread: SharedRef<Thread>,
}

pub struct World {
    processes: BTreeMap<PId, LxProcess>,
}

impl World {
    pub fn new() -> Self {
        let mut processes = BTreeMap::new();
        let pid1 = PId(1);

        let vmspace = SharedRef::new(VmSpace::new().unwrap()).unwrap();

        let entry = UAddr::new(0x1000);
        let image = include_bytes!("hello.bin");
        let vma = VmArea::new_anonymous(align_up(image.len(), arch::MIN_PAGE_SIZE)).unwrap();
        vma.write(0, image).unwrap();

        let attrs = PageAttrs::READ | PageAttrs::EXEC;
        vmspace.map(vma, entry, attrs).unwrap();

        let sp = UAddr::new(0xdead_dead_0000_dead);
        let thread = Thread::new(vmspace.clone(), entry, sp).unwrap();

        thread.start().unwrap();

        processes.insert(pid1, LxProcess { id: pid1, thread });
        Self { processes }
    }
}
