use core::any::Any;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::EventBody;
use ftl_types::sink::EventType;

use crate::process::HandleTable;
use crate::shared_ref::SharedRef;
use crate::sink::EventEmitter;
use crate::syscall::SyscallResult;
use crate::thread::Thread;

/// A set of allowed operations on a kernel object.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HandleRight(u8);

impl HandleRight {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const ALL: Self = Self(Self::READ.0 | Self::WRITE.0);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// A reference to a kernel object and allowed operations on it.
pub struct Handle<T: Handleable + ?Sized> {
    object: SharedRef<T>,
    rights: HandleRight,
}

impl<T: Handleable + ?Sized> Handle<T> {
    pub fn new(object: SharedRef<T>, rights: HandleRight) -> Self {
        Self { object, rights }
    }

    pub fn authorize(self, required: HandleRight) -> Result<SharedRef<T>, ErrorCode> {
        if self.rights.contains(required) {
            Ok(self.object)
        } else {
            Err(ErrorCode::NotAllowed)
        }
    }
}

impl<T: Handleable + ?Sized> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            object: self.object.clone(),
            rights: self.rights,
        }
    }
}

/// A reference to any kernel object.
#[derive(Clone)]
pub struct AnyHandle(Handle<dyn Handleable>);

impl AnyHandle {
    pub fn downcast<T: Handleable>(self) -> Option<Handle<T>> {
        let object = self.0.object.downcast().ok()?;
        let rights = self.0.rights;
        Some(Handle { object, rights })
    }

    pub fn authorize(self, required: HandleRight) -> Result<SharedRef<dyn Handleable>, ErrorCode> {
        self.0.authorize(required)
    }

    pub fn bypass_check(&self) -> &SharedRef<dyn Handleable> {
        &self.0.object
    }
}

impl<T: Handleable> From<Handle<T>> for AnyHandle {
    fn from(handle: Handle<T>) -> Self {
        AnyHandle(Handle {
            object: handle.object,
            rights: handle.rights,
        })
    }
}

pub trait Handleable: Any + Send + Sync {
    fn set_event_emitter(&self, _emitter: Option<EventEmitter>) -> Result<(), ErrorCode> {
        Err(ErrorCode::Unsupported)
    }

    fn close(&self) {
        // Do nothing by default.
    }

    fn read_event(
        &self,
        _handle_table: &mut HandleTable,
    ) -> Result<Option<(EventType, EventBody)>, ErrorCode> {
        Err(ErrorCode::Unsupported)
    }
}

pub fn sys_handle_close(
    current: &SharedRef<Thread>,
    a0: usize,
) -> Result<SyscallResult, ErrorCode> {
    let handle_id = HandleId::from_raw(a0);

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let handle = handle_table.remove(handle_id)?;
    handle.0.object.close();

    Ok(SyscallResult::Return(0))
}
