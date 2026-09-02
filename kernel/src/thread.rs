use core::cell::UnsafeCell;
use core::mem::offset_of;
use core::mem::size_of;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;
use ftl_types::thread::SyscallRegs;
use ftl_utils::spinlock::SpinLock;
use ftl_utils::static_assert;

use crate::address::UAddr;
use crate::address::USlice;
use crate::arch;
use crate::arch::USER_ADDR_END;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::isolate::Isolate;
use crate::poll::Poll;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::SharedRef;
use crate::syscall::SyscallOutput;
use crate::vmspace::VmSpace;

enum State {
    NotStarted,
    Runnable,
    Blocked(SharedRef<Poll>),
    Exited,
}

struct Mutable {
    state: State,
}

#[repr(C)]
pub struct Thread {
    /// The arch-specific CPU registers and other state.
    ///
    /// This is an [`UnsafeCell`] because the interrupt handler updates this
    /// field directly.
    arch: UnsafeCell<arch::Thread>,
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
unsafe impl Sync for Thread {}

impl Thread {
    pub fn new(
        isolate: SharedRef<Isolate>,
        vmspace: SharedRef<VmSpace>,
        pc: usize,
        sp: usize,
        fault_pc: usize,
        cookie: usize,
    ) -> Result<SharedRef<Self>, ErrorCode> {
        // SYSRET-ing to the kernel pages should trigger a page fault, but it
        // is obviously invalid. Reject it early.
        if fault_pc >= USER_ADDR_END {
            return Err(ErrorCode::InvalidArg);
        }

        let mutable = Mutable {
            state: State::NotStarted,
        };

        let arch_thread = arch::Thread::new(pc, sp, fault_pc, cookie)?;
        let thread = SharedRef::new(Thread {
            arch: UnsafeCell::new(arch_thread),
            isolate,
            vmspace,
            mutable: SpinLock::new(mutable),
        })?;

        Ok(thread)
    }

    pub fn arch(&self) -> &UnsafeCell<arch::Thread> {
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

    pub fn isolate(&self) -> &SharedRef<Isolate> {
        &self.isolate
    }

    pub fn start_polling(
        self: &SharedRef<Self>,
        current_thread: &CurrentThread,
        poll: SharedRef<Poll>,
    ) -> Result<SyscallOutput, ErrorCode> {
        let mut mutable = self.mutable.lock();
        if !matches!(mutable.state, State::Runnable) {
            return Err(ErrorCode::InvalidState);
        }

        match poll.try_wait(self)? {
            Some(output) => Ok(SyscallOutput::Done(output.as_raw())),
            None => {
                mutable.state = State::Blocked(poll);

                // Avoid enqueuing this thread twice: in Poll::enqueue (by
                // another CPU), and in return_to_user (by us).
                current_thread.clear();

                Ok(SyscallOutput::Blocked)
            }
        }
    }

    /// Checks if the blocked thread can be woken up, and resume if so.
    pub fn try_wake(self: &SharedRef<Self>) {
        let mut mutable = self.mutable.lock();
        let State::Blocked(ref poll) = mutable.state else {
            return;
        };

        let retval = match poll.try_wait(self) {
            Ok(Some(event)) => event.as_raw(),
            Ok(None) => return,
            Err(err) => err.as_usize(),
        };

        let arch_thread = unsafe { &mut *self.arch.get() };
        arch_thread.set_syscall_retval(retval);
        mutable.state = State::Runnable;
    }

    fn resume_locked(self: &SharedRef<Self>, mutable: &mut Mutable) -> Result<(), ErrorCode> {
        SCHEDULER.push_back(self.clone())?;
        mutable.state = State::Runnable;
        Ok(())
    }

    /// Starts the thread.
    pub fn start(self: &SharedRef<Self>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if !matches!(mutable.state, State::NotStarted) {
            return Err(ErrorCode::InvalidState);
        }

        self.resume_locked(&mut mutable)
    }

    pub fn exit(&self) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if !matches!(mutable.state, State::Runnable) {
            return Err(ErrorCode::InvalidState);
        }

        mutable.state = State::Exited;
        Ok(())
    }

    pub fn write_regs(&self, kind: RegsKind, regs: USlice) -> Result<(), ErrorCode> {
        let mutable = self.mutable.lock();
        if !matches!(mutable.state, State::NotStarted) {
            return Err(ErrorCode::InvalidState);
        }

        // SAFETY: The thread is blocked and we hold the mutable lock to prevent
        //         concurrent access.
        unsafe { &mut *self.arch.get() }.write_regs(kind, regs)?;
        Ok(())
    }

    pub fn write_current_regs(
        &self,
        current_arch: &mut arch::Thread,
        kind: RegsKind,
        regs: USlice,
    ) -> Result<(), ErrorCode> {
        debug_assert!(core::ptr::eq(self.arch.get(), current_arch));
        current_arch.write_regs(kind, regs)
    }

    /// Copies the current thread's registers to another thread.
    ///
    /// The system call entry already saved the current thread's state. No need
    /// to copy any more registers.
    pub fn copy_regs_from_current(
        &self,
        current_arch: &arch::Thread,
        kind: RegsKind,
    ) -> Result<(), ErrorCode> {
        let mutable = self.mutable.lock();
        if !matches!(mutable.state, State::NotStarted) {
            return Err(ErrorCode::InvalidState);
        }

        // SAFETY: The thread is blocked and we hold the mutable lock to prevent
        // concurrent access.
        unsafe { &mut *self.arch.get() }.copy_regs(current_arch, kind)
    }
}

impl Handleable for Thread {}

pub fn sys_thread_create(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let isolate_id = HandleId::new(ctx.a0);
    let vmspace_id = HandleId::new(ctx.a1);
    let pc = ctx.a2;
    let sp = ctx.a3;
    let fault_pc = ctx.a4;
    let cookie = ctx.a5;

    let handle_table = current.isolate().handles();
    let handles = handle_table.lock();
    let isolate = handles.get::<Isolate>(isolate_id, HandleRight::WRITE)?;
    let vmspace = handles.get::<VmSpace>(vmspace_id, HandleRight::READ)?;
    drop(handles);

    let thread = Thread::new(isolate, vmspace, pc, sp, fault_pc, cookie)?;
    let rights = HandleRight::READ | HandleRight::WRITE;
    let handle = Handle::new(thread, rights);
    let id = handle_table.lock().insert(handle)?;
    Ok(SyscallOutput::Done(id.as_usize()))
}

pub fn sys_thread_start(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let thread_id = HandleId::new(ctx.a0);
    let thread = current
        .isolate()
        .handles()
        .lock()
        .get::<Thread>(thread_id, HandleRight::WRITE)?;
    thread.start()?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_thread_write_regs(
    current: &SharedRef<Thread>,
    current_arch: &mut arch::Thread,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let thread_id = HandleId::new(ctx.a0);
    let kind = RegsKind::from_usize(ctx.a1).ok_or(ErrorCode::InvalidArg)?;
    let regs = USlice::new(UAddr::new(ctx.a2), size_of::<Regs>())?;
    let thread = current
        .isolate()
        .handles()
        .lock()
        .get::<Thread>(thread_id, HandleRight::WRITE)?;

    if SharedRef::eq(&thread, current) {
        thread.write_current_regs(current_arch, kind, regs)?;
    } else {
        thread.write_regs(kind, regs)?;
    }

    Ok(SyscallOutput::Done(0))
}

pub fn sys_thread_copy_regs(
    current: &SharedRef<Thread>,
    current_arch: &arch::Thread,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let src_id = HandleId::new(ctx.a0);
    let dest_id = HandleId::new(ctx.a1);
    let kind = RegsKind::from_usize(ctx.a2).ok_or(ErrorCode::InvalidArg)?;
    let handles = current.isolate().handles().lock();
    let src = handles.get::<Thread>(src_id, HandleRight::READ)?;
    let dest = handles.get::<Thread>(dest_id, HandleRight::WRITE)?;
    drop(handles);

    if SharedRef::eq(&src, &dest) {
        return Err(ErrorCode::InvalidArg);
    }

    if !SharedRef::eq(&src, current) {
        // TODO: How should the lock ordering be for src/dest threads?
        return Err(ErrorCode::Unsupported);
    }

    dest.copy_regs_from_current(current_arch, kind)?;
    Ok(SyscallOutput::Done(0))
}

pub fn sys_thread_exit(
    current: &SharedRef<Thread>,
    ctx: &SyscallRegs,
) -> Result<SyscallOutput, ErrorCode> {
    let _reason = ctx.a0; // ignored for now

    current.exit()?;
    Ok(SyscallOutput::Exited)
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
