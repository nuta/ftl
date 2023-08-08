use core::num::NonZeroUsize;

use crate::{
    address::UAddr,
    arch,
    ref_count::{SharedRef, UniqueRef},
    thread::Thread,
};

/// A reference to a kernel object with associated rights, aka *capability*.
///
/// This enum represents all objects that userland can control.
pub enum Handle {
    Free,
    Thread(SharedRef<Thread>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HandleId(NonZeroUsize);

impl HandleId {
    pub const fn new(index: NonZeroUsize) -> HandleId {
        HandleId(index)
    }

    pub fn index(self) -> usize {
        self.0.get() - 1
    }
}

/// The process control block (PCB).
///
/// A process is a collection of threads and resources (page tables and handles)
/// that are shared among the threads.
pub struct Process {
    page_table: UniqueRef<arch::PageTable>,

    // We want to keep the size of `Process` small so that a process can be
    // created as cheaply as possible. When we come to a point where we need
    // more handles, let's consider adding a second-level handle table,
    // similar to indirect blocks in a file system.
    handles: [Handle; 128],
}

impl Process {
    /// Creates a new process.
    pub fn new(page_table: UniqueRef<arch::PageTable>) -> Process {
        const HANDLE_INIT: Handle = Handle::Free;
        Process {
            page_table,
            handles: [HANDLE_INIT; 128],
        }
    }

    pub fn page_table(&self) -> &UniqueRef<arch::PageTable> {
        &self.page_table
    }

    pub fn get_handle(&self, id: HandleId) -> Option<&Handle> {
        self.handles.get(id.index())
    }

    pub fn set_handle(
        &mut self,
        id: HandleId,
        handle: Handle,
    ) -> Result<(), ()> {
        if id.index() >= self.handles.len() {
            return Err(()); // TODO: correct error
        }

        if !matches!(self.handles[id.index()], Handle::Free) {
            return Err(()); // TODO: correct error
        }

        self.handles[id.index()] = handle;
        Ok(())
    }
}
