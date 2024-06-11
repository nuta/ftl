use ftl_types::error::FtlError;

use crate::handle::AnyHandle;
use crate::handle::Handle;
use crate::handle::HandleId;
use crate::handle::HandleRights;
use crate::handle::HandleTable;
use crate::handle::Handleable;
use crate::ref_counted::SharedRef;
use crate::thread::Thread;

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
