
use crate::{ arch, ref_count::Ref, object::{KernelObject, }};

/// Allowed operations on a handle.
///
/// TODO: Use bitfields to save space.
pub struct Rights {
    pub read: bool,
    pub write: bool,
}

/// A reference to a kernel object with associated rights, aka *capability*.
pub struct Handle {
    object: Ref<dyn KernelObject>,
    rights: Rights,
}

/// The process control block (PCB).
///
/// A process is a collection of threads and resources (page tables and handles)
/// that are shared among the threads.
pub struct Process {
    page_table: arch::PageTable,

// We want to keep the size of `Process` small so that a process can be
// created as cheaply as possible. When we come to a point where we need
// more handles, let's consider adding a second-level handle table,
// similar to indirect blocks in a file system.
handles: [Handle; 128],
}
