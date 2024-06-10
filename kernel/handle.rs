use core::any::Any;
use core::num::NonZeroIsize;
use core::ops::Deref;

use hashbrown::HashMap;

use crate::ref_counted::SharedRef;

/// A handle ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleId(NonZeroIsize);

impl HandleId {
    pub const fn from_nonzero(id: NonZeroIsize) -> HandleId {
        Self(id)
    }
}

/// Allowed operations on a handle.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleRights(u8);

/// A trait for kernel objects that can be referred to by a handle ([`Handle`]).
pub trait Handleable: Any + Sync + Send {}

/// Handle, a reference-counted pointer to a kernel object with allowed
/// operations on it, aka *"capability"*.
///
/// # Reference Counting
///
/// This type uses some atomic operations to keep track of the number of
/// references to the underlying object. [`Ordering`] parameters are chosen
/// to be as relaxed as possible in the fast path, inspired by Rust's `Arc`
/// implementation.
pub struct Handle<T: Any + Send + Sync + ?Sized> {
    object: SharedRef<T>,
    rights: HandleRights,
}

impl<T: Handleable> Handle<T> {
    /// Creates a new `Handle` to the given value.
    pub fn new(object: SharedRef<T>, rights: HandleRights) -> Handle<T> {
        Self { object, rights }
    }
}

impl Handle<dyn Any + Sync + Send> {
    pub fn downcast<T: Handleable>(self) -> Result<Handle<T>, Self> {
        match self.object.downcast::<T>() {
            Ok(downcasted) => {
                Ok(Handle {
                    object: downcasted,
                    rights: self.rights,
                })
            }
            Err(original) => {
                Err(Handle {
                    object: original,
                    rights: self.rights,
                })
            }
        }
    }
}

impl<T: Handleable> Deref for Handle<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.object
    }
}

unsafe impl<T: Handleable + ?Sized> Sync for Handle<T> {}
unsafe impl<T: Handleable + ?Sized> Send for Handle<T> {}

pub struct HandleTable {
    handles: HashMap<HandleId, Handle<dyn Handleable>>,
}
