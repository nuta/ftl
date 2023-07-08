use crate::{thread::Thread, arch};

pub struct Handle {}

pub struct HandleTable {}

pub struct Process {
    page_table: arch::PageTable,
    threads: LinkedList<Thread>,
    handles: [Handle; 256],
    indirect_handles: [HandleTable; 16],
}
