use core::any::Any;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::thread::SyscallRegs;

use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

pub trait Handleable: Any + Send + Sync {}

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

pub fn sys_handle_close(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_id = HandleId::new(ctx.a0);
    let handle = current.isolate().handles().lock().remove(handle_id)?;
    drop(handle);
    Ok(SyscallOutput::Done(0))
}
