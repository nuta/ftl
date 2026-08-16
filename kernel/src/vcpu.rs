use core::cell::UnsafeCell;
use core::mem::offset_of;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleRight;
use ftl_utils::spinlock::SpinLock;
use ftl_utils::static_assert;

use crate::arch;
use crate::isolate::Isolate;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::Handleable;
use crate::shared_ref::SharedRef;
use crate::vmspace::VmSpace;

#[derive(Debug, PartialEq, Eq)]
enum State {
    Runnable,
    Blocked,
    Terminated,
}

struct Mutable {
    state: State,
}

#[repr(C)]
pub struct VCpu {
    /// The arch-specific CPU registers and other state.
    ///
    /// This is an [`UnsafeCell`] because the interrupt handler updates this
    /// field directly.
    arch: UnsafeCell<arch::VCpu>,
    isolate: SharedRef<Isolate>,
    vmspace: SharedRef<VmSpace>,
    mutable: SpinLock<Mutable>,
}

/// SAFETY: The `arch` field is accessed when:
///
/// - The CPU is currently running the thread. No other CPUs can run the same
///   thread.
/// - When the thread is blocked (i.e. not running), the mutable lock must be
///   held to prevent concurrent access.
unsafe impl Sync for VCpu {}

impl VCpu {
    pub fn new(
        isolate: SharedRef<Isolate>,
        vmspace: SharedRef<VmSpace>,
        pc: usize,
        sp: usize,
    ) -> Result<SharedRef<Self>, ErrorCode> {
        let mutable = Mutable {
            state: State::Blocked,
        };

        let thread = SharedRef::new(VCpu {
            arch: UnsafeCell::new(arch::VCpu::new(pc, sp)),
            isolate,
            vmspace,
            mutable: SpinLock::new(mutable),
        })?;

        Ok(thread)
    }

    pub fn arch(&self) -> &UnsafeCell<arch::VCpu> {
        &self.arch
    }

    pub fn is_runnable(&self) -> bool {
        // TODO: Avoid locking the spin lock.
        let mutable = self.mutable.lock();
        matches!(mutable.state, State::Runnable)
    }

    pub fn vmspace(&self) -> &SharedRef<VmSpace> {
        &self.vmspace
    }

    /// Resumes the vCPU.
    pub fn unblock(self: &SharedRef<Self>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if mutable.state != State::Blocked {
            return Err(ErrorCode::INVALID_STATE);
        }

        SCHEDULER.push_back(self.clone())?;
        mutable.state = State::Runnable;

        Ok(())
    }
}

impl Handleable for VCpu {
    const DEFAULT_RIGHT: HandleRight = HandleRight::READ.or(HandleRight::WRITE);
}

/// The current vCPU.
///
/// This is a special struct replacing SharedRef<VCpu> for the current
/// vCPU to implement its tricky properties:
///
/// - The offset 0 of this struct is the pointer to `VCpu` and `arch::VCpu`
///   This allows accessing the `arch::VCpu` struct from assembly code to save
///   general-purpose registers.
///
/// - The vCPU running on a CPU should never be dropped. This struct owns a
///   reference count of SharedRef<VCpu>.
#[repr(transparent)]
pub struct CurrentVCpu {
    ptr: UnsafeCell<*const VCpu>,
}

impl CurrentVCpu {
    pub fn new() -> Self {
        Self {
            ptr: UnsafeCell::new(core::ptr::null()),
        }
    }

    /// Clears the current thread.
    pub fn clear(&self) {
        let old_ptr = unsafe { self.ptr.replace(core::ptr::null()) };

        // Release the ref count of the previous vCPU.
        if !old_ptr.is_null() {
            drop(unsafe { SharedRef::from_raw(old_ptr) });
        }
    }

    /// Returns the current vCPU.
    pub fn vcpu(&self) -> Option<SharedRef<VCpu>> {
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
    unsafe fn arch_vcpu(&self) -> *mut arch::VCpu {
        static_assert!(offset_of!(VCpu, arch) == 0);
        debug_assert!(!unsafe { *self.ptr.get() }.is_null());

        // SAFETY: The static_assert above guarantees arch::VCpu is at the offset 0.
        unsafe { *self.ptr.get() as *mut arch::VCpu }
    }

    /// Updates the current vCPU.
    fn update(&self, next: SharedRef<VCpu>) {
        let new_ptr = next.into_raw();

        // SAFETY: Data races should not happen because this is CPU-local and
        //         interrupts are disabled.
        let old_ptr = unsafe { self.ptr.replace(new_ptr) };

        // Decrement the ref count of the current vCPU.
        if !old_ptr.is_null() {
            drop(unsafe { SharedRef::from_raw(old_ptr) });
        }
    }

    /// Switches into a new vCPU.
    ///
    /// # Warning
    ///
    /// Drop reference counters and lock guards before calling this; this
    /// function never returns.
    pub fn enter(&self, new_vcpu: SharedRef<VCpu>) -> ! {
        // Switch to the new vCPU's virtual memory space.
        new_vcpu.vmspace().switch();

        self.update(new_vcpu);

        // SAFETY: We've set the new pointer and SharedRef is always non-null.
        let arch_vcpu = unsafe { self.arch_vcpu() };

        arch::VCpu::enter(arch_vcpu);
    }
}
