use core::cell::UnsafeCell;
use core::mem::offset_of;

use ftl_api::error::ErrorCode;
use ftl_api::handle::HandleRight;
use ftl_api::thread::ContextData;
use ftl_api::thread::ContextKind;
use ftl_api::thread::UpcallArg;
use ftl_api::upcall::Upcall;
use ftl_utils::spinlock::SpinLock;
use ftl_utils::static_assert;

use crate::arch;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::Handleable;
use crate::shared_ref::SharedRef;
use crate::vmspace::VmSpace;

#[derive(Debug, PartialEq, Eq)]
enum State {
    Runnable,
    Blocked,
}

struct Mutable {
    state: State,
}

#[repr(C)]
pub struct Thread {
    arch: arch::Thread,
    upcall: Upcall<UpcallArg>,
    vmspace: SharedRef<VmSpace>,
    mutable: SpinLock<Mutable>,
}

impl Thread {
    pub fn new(
        vmspace: SharedRef<VmSpace>,
        upcall: Upcall<UpcallArg>,
    ) -> Result<SharedRef<Self>, ErrorCode> {
        let mutable = Mutable {
            state: State::Blocked,
        };

        let thread = SharedRef::new(Thread {
            arch: arch::Thread::new(),
            vmspace,
            upcall,
            mutable: SpinLock::new(mutable),
        })?;

        Ok(thread)
    }

    pub fn is_runnable(&self) -> bool {
        // TODO: Avoid locking the spin lock.
        let mutable = self.mutable.lock();
        matches!(mutable.state, State::Runnable)
    }

    pub fn vmspace(&self) -> &SharedRef<VmSpace> {
        &self.vmspace
    }

    pub fn arch(&self) -> &arch::Thread {
        &self.arch
    }

    pub fn upcall(&self, arg: UpcallArg) {
        self.upcall.invoke(arg);
    }

    /// Marks the thread as blocked. It will not be scheduled for execution
    /// until [`Self::unblock`] is called.
    pub fn block(self: &SharedRef<Self>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.state != State::Runnable {
            return Err(ErrorCode::INVALID_STATE);
        }

        mutable.state = State::Blocked;
        Ok(())
    }

    /// Resumes the thread.
    pub fn unblock(self: &SharedRef<Self>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.state != State::Blocked {
            return Err(ErrorCode::INVALID_STATE);
        }

        SCHEDULER.push_back(self.clone())?;
        mutable.state = State::Runnable;

        Ok(())
    }

    /// Reads the thread's context such as general-purpose registers.
    pub fn read_context(&self, kind: ContextKind, regs: &mut ContextData) -> Result<(), ErrorCode> {
        let mutable = self.mutable.lock();
        if mutable.state != State::Blocked {
            return Err(ErrorCode::INVALID_STATE);
        }

        self.arch.read_context(kind, regs);
        Ok(())
    }

    /// Writes the thread's context such as general-purpose registers.
    pub fn write_context(&self, kind: ContextKind, regs: &ContextData) -> Result<(), ErrorCode> {
        let mutable = self.mutable.lock();
        if mutable.state != State::Blocked {
            return Err(ErrorCode::INVALID_STATE);
        }

        unsafe {
            // FIXME: Use UnsafeCell?
            (*(&self.arch as *const arch::Thread as *mut arch::Thread)).write_context(kind, regs)
        }
        Ok(())
    }
}

impl Handleable for Thread {
    const DEFAULT_RIGHT: HandleRight = HandleRight::READ.or(HandleRight::WRITE);
}

/// The current thread.
///
/// This is a special struct replacing SharedRef<Thread> for the current
/// thread to implement its tricky properties:
///
/// - The offset 0 of this struct is the pointer to `Thread` and `arch::Thread`
///   This allows accessing the `arch::Thread` struct from assembly code to save
///   general-purpose registers.
///
/// - The thread running on a CPU should never be dropped. This struct owns a
///   reference count of SharedRef<Thread>.
#[repr(transparent)]
pub struct CurrentThread {
    ptr: UnsafeCell<*const Thread>,
}

impl CurrentThread {
    pub fn new() -> Self {
        Self {
            ptr: UnsafeCell::new(core::ptr::null()),
        }
    }

    /// Clears the current thread.
    pub fn clear(&self) {
        let old_ptr = unsafe { self.ptr.replace(core::ptr::null()) };

        // Release the ref count of the previous thread.
        if !old_ptr.is_null() {
            drop(unsafe { SharedRef::from_raw(old_ptr) });
        }
    }

    /// Returns the current thread.
    pub fn thread(&self) -> Option<SharedRef<Thread>> {
        unsafe {
            let ptr = *self.ptr.get();
            if ptr.is_null() {
                return None;
            }

            // Create and clone a temporary ref to increment the reference count.
            let temp = SharedRef::from_raw(ptr);
            let cloned = temp.clone();
            core::mem::forget(temp);

            Some(cloned)
        }
    }

    /// Returns the pointer to the arch-specific thread struct.
    ///
    /// # Safety
    ///
    /// The caller must ensure the current thread is set.
    unsafe fn arch_thread(&self) -> *mut arch::Thread {
        static_assert!(offset_of!(Thread, arch) == 0);
        debug_assert!(!unsafe { *self.ptr.get() }.is_null());

        // SAFETY: The static_assert above guarantees arch::Thread is at the offset 0.
        unsafe { *self.ptr.get() as *mut arch::Thread }
    }

    /// Updates the current thread.
    fn update(&self, next: SharedRef<Thread>) {
        let new_ptr = next.into_raw();

        // SAFETY: Data races should not happen because this is CPU-local and
        //         interrupts are disabled.
        let old_ptr = unsafe { self.ptr.replace(new_ptr) };

        // Decrement the ref count of the current thread.
        if !old_ptr.is_null() {
            drop(unsafe { SharedRef::from_raw(old_ptr) });
        }
    }

    /// Switches into a new thread.
    ///
    /// # Warning
    ///
    /// Drop reference counters and lock guards before calling this; this
    /// function never returns.
    pub fn enter(&self, new_thread: SharedRef<Thread>) -> ! {
        // Switch to the new thread's virtual memory space.
        new_thread.vmspace().switch();

        self.update(new_thread);

        // SAFETY: We've set the new pointer and SharedRef is always non-null.
        let arch_thread = unsafe { self.arch_thread() };

        arch::Thread::enter(arch_thread);
    }
}
