use core::any::Any;
use core::ops::Deref;

use ftl_types::error::FtlError;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRights;
use ftl_utils::downcast::Downcastable;
use hashbrown::HashMap;

use crate::ref_counted::SharedRef;

/// A trait for kernel objects that can be referred to by a handle ([`Handle`]).
pub trait Handleable: Any + Sync + Send {}

/// Handle, a reference-counted pointer to a kernel object with allowed
/// operations on it, aka *"capability"*.
pub struct Handle<T: Handleable + ?Sized> {
    object: SharedRef<T>,
    rights: HandleRights,
}

impl<T: Handleable> Handle<T> {
    pub fn rights(&self) -> HandleRights {
        self.rights
    }
}

impl<T: Handleable> Clone for Handle<T> {
    fn clone(&self) -> Handle<T> {
        Handle {
            object: self.object.clone(),
            rights: self.rights,
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

pub struct AnyHandle(Handle<dyn Handleable>);

impl AnyHandle {
    pub fn new<T: Handleable>(object: SharedRef<T>, rights: HandleRights) -> AnyHandle {
        AnyHandle(Handle {
            object: object as SharedRef<dyn Handleable>,
            rights: rights,
        })
    }

    pub fn downcast<T: Handleable>(&self) -> Option<&Handle<T>> {
        self.0.as_any().downcast_ref::<Handle<T>>()
    }
}

/// The number of maximum handles per process.
///
/// The current 64K limit has no particular reason, but it should be low
/// enough to prevent an overflow in `next_id + 1` in `HandleTable::add`.
///
/// The hard limit is `2^HANDLE_ID_BITS - 1`.
const NUM_HANDLES_MAX: i32 = 64 * 1024;

pub struct HandleTable {
    next_id: i32,
    handles: HashMap<HandleId, AnyHandle>,
}

impl HandleTable {
    pub fn new() -> HandleTable {
        HandleTable {
            next_id: 1,
            handles: HashMap::new(),
        }
    }

    /// Add a handle to the table.
    pub fn add(&mut self, handle: AnyHandle) -> Result<HandleId, FtlError> {
        if self.next_id >= NUM_HANDLES_MAX {
            return Err(FtlError::TooManyHandles);
        }

        let id = HandleId::from_raw(self.next_id);
        self.next_id = self.next_id + 1;
        self.handles.insert(id, handle);
        Ok(id)
    }

    /// Get a handle by ID, as a concrete type `T`.
    pub fn get<T>(&self, id: HandleId) -> Result<&Handle<T>, FtlError>
    where
        T: Handleable,
    {
        println!("HandleTable::get");
        let any_handle = self.handles.get(&id).ok_or(FtlError::HandleNotFound)?;
        println!("HandleTable::get downcast");
        let handle = any_handle
            .downcast()
            .ok_or(FtlError::UnexpectedHandleType)?;
        println!("HandleTable::get downcast Ok");
        Ok(handle)
    }

    pub fn get_owned<T>(&self, id: HandleId) -> Result<Handle<T>, FtlError>
    where
        T: Handleable,
    {
        let handle: &Handle<T> = self.get(id)?;
        Ok(handle.clone())
    }
}
