use core::cell::UnsafeCell;
use core::mem::offset_of;

use ftl_utils::static_assert;

use crate::address::UAddr;
use crate::arch;
use crate::error::ErrorCode;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::vmspace::VmSpace;

#[derive(Debug, PartialEq, Eq)]
enum State {
    Created,
    Runnable,
}

struct Mutable {
    state: State,
}

#[repr(C)]
pub struct Thread {
    arch: arch::Thread,
    mutable: SpinLock<Mutable>,
    vmspace: SharedRef<VmSpace>,
}

impl Thread {
    pub fn new(
        vmspace: SharedRef<VmSpace>,
        entry: UAddr,
        sp: UAddr,
    ) -> Result<SharedRef<Self>, ErrorCode> {
        let mutable = Mutable {
            state: State::Created,
        };

        let thread = SharedRef::new(Thread {
            mutable: SpinLock::new(mutable),
            arch: arch::Thread::new(entry, sp),
            vmspace,
        })?;

        Ok(thread)
    }

    pub fn vmspace(&self) -> &SharedRef<VmSpace> {
        &self.vmspace
    }

    pub fn start(self: &SharedRef<Self>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.state != State::Created {
            return Err(ErrorCode::InvalidState);
        }

        mutable.state = State::Runnable;
        if let Err(e) = SCHEDULER.push_front(self.clone()) {
            return Err(e);
        }

        Ok(())
    }
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

    /// Updates the current thread.
    ///
    fn update(&self, next: SharedRef<Thread>) -> *const arch::Thread {
        let new_ptr = next.into_raw();

        // SAFETY: Data races should not happen because this is CPU-local and
        //         interrupts are disabled.
        let old_ptr = unsafe { self.ptr.replace(new_ptr) };

        // Decrement the ref count of the current thread.
        if !old_ptr.is_null() {
            drop(unsafe { SharedRef::from_raw(old_ptr) });
        }

        // SAFETY: We've set the new pointer and SharedRef is always non-null.
        unsafe { self.arch_thread() }
    }

    /// Clears the current thread.
    pub fn clear(&self) {
        unsafe { self.ptr.replace(core::ptr::null()) };
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
}

/// Jumps to a thread.
///
/// In other words, it leaves the kernel. The kernel will be resumed when
/// an exception or interrupt occurs.
///
/// Unlike traditional operating systems, this function never returns because of
/// the single kernel stack design.
pub fn return_to_user() -> ! {
    let cpuvar = arch::get_cpuvar();
    let current = &cpuvar.current_thread;

    if let Some(current) = current.thread() {
        if matches!(current.mutable.lock().state, State::Runnable) {
            // The current thread is runnable. Push it back to the scheduler.
            SCHEDULER
                .push_front(current)
                .expect("out of memory in runqueue"); // FIXME:
        }
    }

    let Some(thread) = SCHEDULER.pop() else {
        // Clear the current thread. Otherwise, the interrupt handler would
        // overwrite the user's system call context (registers) with the idle
        // thread's context.
        current.clear();

        // No threads to run. Enter the idle loop.
        arch::idle();
    };

    // Switch the address space.
    thread.vmspace().switch();

    // Update the current thread.
    let arch_thread = current.update(thread);

    // Note: Drop references before calling this; this function never returns.
    arch::Thread::enter(arch_thread);
}
