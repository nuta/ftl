use alloc::collections::BTreeMap;
use alloc::collections::btree_map::Entry;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_utils::spinlock::SpinLock;

use crate::handle::AnyHandle;
use crate::handle::Handleable;
use crate::shared_ref::SharedRef;

pub struct Isolate {
    handles: SpinLock<HandleTable>,
}

impl Isolate {
    pub fn new() -> Self {
        Self {
            handles: SpinLock::new(HandleTable::new()),
        }
    }

    pub fn handles(&self) -> &SpinLock<HandleTable> {
        &self.handles
    }
}

impl Handleable for Isolate {}

const NUM_HANDLES_MAX: usize = 1024;

pub struct HandleTable {
    handles: BTreeMap<usize, AnyHandle>,
}

impl HandleTable {
    pub const fn new() -> Self {
        Self {
            handles: BTreeMap::new(),
        }
    }

    pub fn insert<H: Into<AnyHandle>>(&mut self, handle: H) -> Result<HandleId, ErrorCode> {
        for raw_id in 1..=NUM_HANDLES_MAX {
            if let Entry::Vacant(e) = self.handles.entry(raw_id) {
                e.insert(handle.into());
                return Ok(HandleId::new(raw_id));
            }
        }

        Err(ErrorCode::TOO_MANY_HANDLES)
    }

    pub fn insert_at<H: Into<AnyHandle>>(
        &mut self,
        id: HandleId,
        handle: H,
    ) -> Result<(), ErrorCode> {
        let raw_id = id.as_usize();
        if raw_id == 0 || raw_id > NUM_HANDLES_MAX {
            return Err(ErrorCode::INVALID_ARG);
        }

        if self.handles.contains_key(&raw_id) {
            return Err(ErrorCode::ALREADY_EXISTS);
        }

        self.handles.insert(raw_id, handle.into());
        Ok(())
    }

    pub fn get<T: Handleable>(
        &self,
        id: HandleId,
        required: HandleRight,
    ) -> Result<SharedRef<T>, ErrorCode> {
        self.handles
            .get(&id.as_usize())
            .cloned()
            .ok_or(ErrorCode::INVALID_ARG)?
            .downcast::<T>()
            .ok_or(ErrorCode::INVALID_TYPE)?
            .authorize(required)
    }
}
