use core::any::Any;

use ftl_types::error::ErrorCode;

use crate::shared_ref::SharedRef;

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
}

impl<T: Handleable> From<Handle<T>> for AnyHandle {
    fn from(handle: Handle<T>) -> Self {
        AnyHandle(Handle {
            object: handle.object,
            rights: handle.rights,
        })
    }
}

pub trait Handleable: Any + Send + Sync {}
