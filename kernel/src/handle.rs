use core::any::Any;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleRight;

use crate::shared_ref::SharedRef;

pub trait Handleable: Any + Send + Sync {
    /// Called when a handle is explicitly closed.
    fn close(self: SharedRef<Self>) {}
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
    pub fn close(self) {
        self.0.object.close();
    }

    /// Updates the rights of this handle.
    ///
    /// The new rights must be a subset of the current rights.
    pub fn update_rights(self, rights: HandleRight) -> Result<Self, ErrorCode> {
        if !self.0.rights.contains(rights) {
            // The new rights are not a subset.
            return Err(ErrorCode::NotAllowed);
        }

        Ok(AnyHandle(Handle {
            object: self.0.object,
            rights,
        }))
    }

    pub fn downcast<T: Handleable>(self) -> Option<Handle<T>> {
        let object = self.0.object.downcast().ok()?;
        let rights = self.0.rights;
        Some(Handle { object, rights })
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
