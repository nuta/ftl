use core::cell::UnsafeCell;
use core::mem::offset_of;

use ftl_types::error::ErrorCode;
use ftl_types::syscall::ERROR_RETVAL_BASE;
use ftl_utils::static_assert;

use crate::arch;
use crate::isolation::UserSlice;
use crate::process::IDLE_PROCESS;
use crate::process::Process;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::SharedRef;
use crate::sink::Sink;
use crate::spinlock::SpinLock;

pub enum Promise {
    SinkWait {
        sink: SharedRef<Sink>,
        buf: UserSlice,
    },
}

impl Promise {
    pub fn poll(
        &self,
        current: &CurrentThread,
        thread: &SharedRef<Thread>,
    ) -> Option<Result<usize, ErrorCode>> {
        match self {
            Promise::SinkWait { sink, buf } => {
                let process = thread.process();
                let mut handle_table = process.handle_table().lock();
                match sink.wait(thread, process.isolation(), &mut handle_table, buf) {
                    Ok(true) => Some(Ok(0)),
                    Ok(false) => {
                        // Still not ready.
                        None
                    }
                    Err(error) => {
                        unsafe { current.set_syscall_result(Err(error)) };
                        Some(Err(error))
                    }
                }
            }
        }
    }
}

enum State {
    Runnable,
    Blocked(Promise),
    Idle,
}

struct Mutable {
    state: State,
}

#[repr(C)]
pub struct Thread {
    pub arch: arch::Thread,
    process: SharedRef<Process>,
    mutable: SpinLock<Mutable>,
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
            mutable: SpinLock::new(Mutable {
                state: State::Runnable,
            }),
        })
    }

    pub fn new_idle() -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            arch: arch::Thread::new_idle(),
            process: IDLE_PROCESS.clone(),
            mutable: SpinLock::new(Mutable { state: State::Idle }),
        })
    }

    pub fn process(&self) -> &SharedRef<Process> {
        &self.process
    }

    pub fn block_on(&self, promise: Promise) {
        let mut mutable = self.mutable.lock();
        mutable.state = State::Blocked(promise);
    }

    pub fn unblock(self: SharedRef<Self>) {
        // Evaluate the promsie again in Thread::poll.
        SCHEDULER.push(self);
    }

    /// Attempts to resolve the blocked state, and returns `true` if the
    /// thread is now runnable.
    pub fn poll(self: &SharedRef<Self>, current: &CurrentThread) -> bool {
        let mutable = self.mutable.lock();
        match &mutable.state {
            State::Runnable => true,
            State::Idle => false,
            State::Blocked(promise) => {
                if let Some(result) = promise.poll(current, self) {
                    unsafe { current.set_syscall_result(result) };
                    true
                } else {
                    false
                }
            }
        }
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

    /// Sets the system call return value.
    ///
    /// # Safety
    ///
    /// This function must be called only from the CPU which is running the
    /// thread.
    pub unsafe fn set_syscall_result(&self, retval: Result<usize, ErrorCode>) {
        let raw = match retval {
            Ok(retval) if retval >= ERROR_RETVAL_BASE => {
                println!("invalid syscall return value: {:x}", retval);
                ERROR_RETVAL_BASE + ErrorCode::Unreachable as usize
            }
            Ok(retval) => retval,
            Err(error) => ERROR_RETVAL_BASE + error as usize,
        };

        // I wish there was a better way to do this...
        unsafe {
            (*self.arch_thread()).set_syscall_result(raw);
        }
    }
}

fn schedule() -> Option<*const arch::Thread> {
    let cpuvar = arch::get_cpuvar();
    let current = &cpuvar.current_thread;
    let current_thread = current.thread();
    if matches!(current_thread.mutable.lock().state, State::Runnable) {
        // The current thread is runnable. Push it back to the scheduler.
        SCHEDULER.push(current_thread);
    }

    while let Some(thread) = SCHEDULER.pop() {
        if thread.poll(current) {
            current.update(thread);
            let arch_thread = current.arch_thread();
            return Some(arch_thread);
        }
    }

    None
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
