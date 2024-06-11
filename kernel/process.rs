use ftl_types::error::FtlError;

use crate::{handle::{AnyHandle, Handle, HandleId, HandleRights, HandleTable, Handleable}, ref_counted::SharedRef, thread::Thread};

pub struct Process {
    handles: HandleTable,
}

impl Process {
    pub fn create() -> SharedRef<Process> {
        SharedRef::new(Process {
            handles: HandleTable::new(),
        })
    }

    pub fn add_handle(&mut self, handle: AnyHandle) -> Result<HandleId, FtlError> {
        self.handles.add(handle)

    }
}
