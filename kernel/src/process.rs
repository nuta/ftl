use alloc::collections::btree_map::BTreeMap;
use core::cmp::Ordering;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;

use crate::handle::AnyHandle;
use crate::isolation::INKERNEL_ISOLATION;
use crate::isolation::Isolation;
use crate::shared_ref::RefCounted;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;

pub struct Process {
    isolation: SharedRef<dyn Isolation>,
    handle_table: SpinLock<HandleTable>,
}

impl Process {
    pub fn new(isolation: SharedRef<dyn Isolation>) -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            isolation,
            handle_table: SpinLock::new(HandleTable::new()),
        })
    }

    pub fn isolation(&self) -> &SharedRef<dyn Isolation> {
        &self.isolation
    }

    pub fn handle_table(&self) -> &SpinLock<HandleTable> {
        &self.handle_table
    }
}

const NUM_HANDLES_MAX: usize = 1024;

pub struct HandleTable {
    handles: BTreeMap<usize, AnyHandle>,
    next_id: usize,
}

impl HandleTable {
    pub const fn new() -> Self {
        Self {
            handles: BTreeMap::new(),
            next_id: 1,
        }
    }

    pub fn insert<H: Into<AnyHandle>>(&mut self, object: H) -> Result<HandleId, ErrorCode> {
        if self.handles.len() >= NUM_HANDLES_MAX {
            return Err(ErrorCode::TooManyHandles);
        }

        let id = HandleId::from_raw(self.next_id);
        self.next_id += 1;
        self.handles.insert(id.as_usize(), object.into());
        Ok(id)
    }
}

pub static IDLE_PROCESS: SharedRef<Process> = {
    static INNER: RefCounted<Process> = RefCounted::new_static(Process {
        isolation: SharedRef::clone_static(&INKERNEL_ISOLATION),
        handle_table: SpinLock::new(HandleTable::new()),
    });
    let process = SharedRef::new_static(&INNER);
    process as SharedRef<Process>
};
