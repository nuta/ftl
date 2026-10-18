use alloc::vec::Vec;
use core::mem;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HANDLE_ID_MAX;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::thread::SyscallRegs;
use ftl_utils::fxhash::FxHashMap;
use ftl_utils::reserve_slot::ReserveSlot;
use ftl_utils::spinlock::SpinLock;
use ftl_utils::static_assert;

use crate::handle::AnyHandle;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::thread::Thread;

const NUM_HANDLES_MAX: usize = 1024;

static_assert!(NUM_HANDLES_MAX <= HANDLE_ID_MAX);

struct Mutable {
    destroyed: bool,
    threads: Vec<SharedRef<Thread>>,
    handles: FxHashMap<usize, AnyHandle>,
}

/// A handle space.
pub struct HandleSpace {
    mutable: SpinLock<Mutable>,
}

impl HandleSpace {
    pub fn new() -> Self {
        Self {
            mutable: SpinLock::new(Mutable {
                destroyed: false,
                threads: Vec::new(),
                handles: FxHashMap::new(),
            }),
        }
    }

    pub fn add_thread(&self, thread: SharedRef<Thread>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.destroyed {
            return Err(ErrorCode::Destroyed);
        }

        let slot = mutable
            .threads
            .reserve_slot()
            .map_err(|_| ErrorCode::OutOfMemory)?;

        slot.push(thread);
        Ok(())
    }

    pub fn remove_thread(&self, thread: &Thread) {
        self.mutable
            .lock()
            .threads
            .retain(|t| !core::ptr::eq(t.as_ptr(), thread));
    }

    pub fn insert<H: Into<AnyHandle>>(&self, handle: H) -> Result<HandleId, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let id = Self::find_free_id(&mutable)?;
        self.do_insert(&mut mutable, id, handle)?;
        Ok(id)
    }

    /// Moves a handle to another handle space.
    pub fn do_move(
        &self,
        id: HandleId,
        dst: &HandleSpace,
        rights: HandleRight,
    ) -> Result<HandleId, ErrorCode> {
        if core::ptr::eq(self, dst) {
            // The same handle space.
            return Err(ErrorCode::InvalidArg);
        }

        let (mut src_mutable, mut dst_mutable) = SpinLock::lock_two(&self.mutable, &dst.mutable);

        let handle = src_mutable
            .handles
            .get(&id.as_usize())
            .cloned()
            .ok_or(ErrorCode::HandleNotFound)?;

        let handle = handle.update_rights(rights)?;

        let new_id = Self::find_free_id(&dst_mutable)?;
        dst.do_insert(&mut dst_mutable, new_id, handle)?;
        src_mutable.handles.remove(&id.as_usize());
        Ok(new_id)
    }

    pub fn duplicate(&self, id: HandleId, rights: HandleRight) -> Result<HandleId, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let handle = mutable
            .handles
            .get(&id.as_usize())
            .cloned()
            .ok_or(ErrorCode::HandleNotFound)?
            .update_rights(rights)?;

        let new_id = Self::find_free_id(&mutable)?;
        self.do_insert(&mut mutable, new_id, handle)?;
        Ok(new_id)
    }

    fn find_free_id(mutable: &Mutable) -> Result<HandleId, ErrorCode> {
        for raw_id in 1..=NUM_HANDLES_MAX {
            if !mutable.handles.contains_key(&raw_id) {
                return Ok(HandleId::new(raw_id));
            }
        }

        Err(ErrorCode::TooManyHandles)
    }

    pub fn insert_at<H: Into<AnyHandle>>(&self, id: HandleId, handle: H) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        self.do_insert(&mut mutable, id, handle)
    }

    fn do_insert<H: Into<AnyHandle>>(
        &self,
        mutable: &mut Mutable,
        id: HandleId,
        handle: H,
    ) -> Result<(), ErrorCode> {
        if mutable.destroyed {
            return Err(ErrorCode::Destroyed);
        }

        let raw_id = id.as_usize();
        if raw_id == 0 || raw_id > NUM_HANDLES_MAX {
            return Err(ErrorCode::InvalidHandleId);
        }

        if mutable.handles.contains_key(&raw_id) {
            return Err(ErrorCode::AlreadyExists);
        }

        mutable
            .handles
            .reserve_slot()
            .map_err(|_| ErrorCode::OutOfMemory)?
            .insert(raw_id, handle.into());
        Ok(())
    }

    pub fn get<T: Handleable>(
        &self,
        id: HandleId,
        required: HandleRight,
    ) -> Result<SharedRef<T>, ErrorCode> {
        let mutable = self.mutable.lock();
        mutable
            .handles
            .get(&id.as_usize())
            .cloned()
            .ok_or(ErrorCode::HandleNotFound)?
            .downcast::<T>()
            .ok_or(ErrorCode::InvalidHandleType)?
            .authorize(required)
    }

    pub fn get2<T1: Handleable, T2: Handleable>(
        &self,
        id1: HandleId,
        required1: HandleRight,
        id2: HandleId,
        required2: HandleRight,
    ) -> Result<(SharedRef<T1>, SharedRef<T2>), ErrorCode> {
        let mutable = self.mutable.lock();
        let Some(handle1) = mutable.handles.get(&id1.as_usize()) else {
            return Err(ErrorCode::HandleNotFound);
        };

        let Some(handle2) = mutable.handles.get(&id2.as_usize()) else {
            return Err(ErrorCode::HandleNotFound);
        };

        let handle1 = handle1
            .clone()
            .downcast::<T1>()
            .ok_or(ErrorCode::InvalidHandleType)?
            .authorize(required1)?;
        let handle2 = handle2
            .clone()
            .downcast::<T2>()
            .ok_or(ErrorCode::InvalidHandleType)?
            .authorize(required2)?;
        Ok((handle1, handle2))
    }

    pub fn remove(&self, id: HandleId) -> Result<AnyHandle, ErrorCode> {
        let mut mutable = self.mutable.lock();
        mutable
            .handles
            .remove(&id.as_usize())
            .ok_or(ErrorCode::HandleNotFound)
    }
}

impl Handleable for HandleSpace {
    fn close(self: SharedRef<Self>) {
        let (threads, mut handles) = {
            let mut mutable = self.mutable.lock();
            mutable.destroyed = true;
            (
                mem::take(&mut mutable.threads),
                mem::take(&mut mutable.handles),
            )
        };

        // Terminate threads.
        for thread in threads {
            thread.close();
        }

        for (_, handle) in handles.drain() {
            handle.close();
        }
    }
}

pub fn sys_hspace_create(
    current: &SharedRef<Thread>,
    _ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let current_hspace = current.hspace();
    let new_hspace = SharedRef::new(HandleSpace::new())?;
    let handle = Handle::new(new_hspace.clone(), HandleRight::WRITE);
    let id = match current_hspace.insert(handle) {
        Ok(id) => id,
        Err(err) => {
            new_hspace.close();
            return Err(err);
        }
    };
    Ok(SyscallOutput::Done(id.as_usize()))
}

pub fn sys_hspace_move(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let dst_id = HandleId::new(ctx.a0);
    let handle_id = HandleId::new(ctx.a1);
    let rights = HandleRight::from_raw(ctx.a2)?;

    let current_hspace = current.hspace();
    let dst = current_hspace.get::<HandleSpace>(dst_id, HandleRight::WRITE)?;
    let new_id = current_hspace.do_move(handle_id, &dst, rights)?;
    Ok(SyscallOutput::Done(new_id.as_usize()))
}

pub fn sys_hspace_duplicate(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_id = HandleId::new(ctx.a0);
    let rights = HandleRight::from_raw(ctx.a1)?;

    let new_id = current.hspace().duplicate(handle_id, rights)?;
    Ok(SyscallOutput::Done(new_id.as_usize()))
}

pub fn sys_hspace_close(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let handle_id = HandleId::new(ctx.a0);
    let handle = current.hspace().remove(handle_id)?;
    handle.close();
    Ok(SyscallOutput::Done(0))
}
