use crate::{thread::Thread, arch, ref_count::Ref, object::KernelObject};

pub struct Handle {
    object: Ref<dyn KernelObject>,
}

pub struct Process {
    page_table: arch::PageTable,
    handles: [Handle; 256],
    //  TODO:
    // indirect_handles: [HandleTable; 16],
}
