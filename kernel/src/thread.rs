use core::cell::UnsafeCell;
use core::mem::offset_of;

use ftl_types::error::ErrorCode;
use ftl_utils::static_assert;

use crate::arch;
use crate::process::IDLE_PROCESS;
use crate::process::Process;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::SharedRef;

#[repr(C)]
pub struct Thread {
    pub arch: arch::Thread,
    process: SharedRef<Process>,
}

impl Thread {
    pub fn new(
        process: SharedRef<Process>,
        entry: usize,
        sp: usize,
        start_info: usize,
    ) -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            arch: arch::Thread::new(entry, sp, start_info),
            process,
        })
    }

    pub fn new_idle() -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            arch: arch::Thread::new_idle(),
            process: IDLE_PROCESS.clone(),
        })
    }

    pub fn process(&self) -> &SharedRef<Process> {
        &self.process
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
    pub fn new(idle_thread: SharedRef<Thread>) -> Self {
        Self {
            ptr: UnsafeCell::new(idle_thread.into_raw()),
        }
    }

    /// Updates the current thread.
    ///
    fn update(&self, next: SharedRef<Thread>) {
        let new_ptr = next.into_raw();

        // SAFETY: Data races should not happen because this is CPU-local and
        //         interrupts are disabled.
        let old_ptr = unsafe { self.ptr.replace(new_ptr) };

        // Decrement the ref count of the current thread.
        drop(unsafe { SharedRef::from_raw(old_ptr) });
    }

    /// Returns the current thread.
    pub fn thread(&self) -> SharedRef<Thread> {
        unsafe {
            let ptr = *self.ptr.get();

            // Create and clone a temporary ref to increment the reference count.
            let temp = SharedRef::from_raw(ptr);
            let cloned = temp.clone();
            core::mem::forget(temp);

            cloned
        }
    }

    /// Returns the pointer to the arch-specific thread struct.
    fn arch_thread(&self) -> *mut arch::Thread {
        static_assert!(offset_of!(Thread, arch) == 0);

        // SAFETY: The static_assert above guarantees arch::Thread is at the offset 0.
        unsafe { *self.ptr.get() as *mut arch::Thread }
    }
}

fn schedule() -> Option<*const arch::Thread> {
    let thread = SCHEDULER.pop()?;
    let cpuvar = arch::get_cpuvar();

    cpuvar.current_thread.update(thread);

    let arch_thread = cpuvar.current_thread.arch_thread();
    Some(arch_thread)
}

/// Jumps to a thread.
///
/// In other words, it leaves the kernel. The kernel will be resumed when
/// an exception or interrupt occurs.
///
/// Unlike traditional operating systems, this function never returns due to
/// its single stack design.
pub fn return_to_user() -> ! {
    let Some(thread) = schedule() else {
        // No threads to run. Enter the idle loop.
        arch::idle();
    };

    arch::thread_switch(thread);
}
