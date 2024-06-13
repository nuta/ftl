use ftl_types::error::FtlError;

use crate::handle::AnyHandle;
use crate::handle::HandleId;
use crate::handle::HandleTable;
use crate::ref_counted::SharedRef;

pub struct Process {
    handles: HandleTable,
}

impl Process {
    pub fn create() -> Process {
        Process {
            handles: HandleTable::new(),
        }
    }

    pub fn add_handle(&mut self, handle: AnyHandle) -> Result<HandleId, FtlError> {
        self.handles.add(handle)
    }
}
