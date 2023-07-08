use core::mem::size_of;

use utils::static_assert;

use crate::{thread::Thread, arch::{self, PAGE_SIZE}, ref_count::Ref, object::KernelObject};

pub struct Handle {
    object: Ref<dyn KernelObject>,
}

pub struct Process {
    page_table: arch::PageTable,
    handles: [Handle; 256],
    //  TODO:
    // indirect_handles: [HandleTable; 16],
}

static_assert!(size_of::<Process>() <= PAGE_SIZE);
