use alloc::collections::vec_deque::VecDeque;
use core::cell::UnsafeCell;
use core::mem::offset_of;

use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::sink::Event;
use ftl_types::sink::EventHeader;
use ftl_types::sink::EventType;
use ftl_types::sink::SandboxedSyscallEvent;
use ftl_types::sink::SyscallRegs;
use ftl_types::syscall::ERROR_RETVAL_BASE;
use ftl_utils::static_assert;

use crate::arch;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::UserSlice;
use crate::process::HandleTable;
use crate::process::IDLE_PROCESS;
use crate::process::Process;
use crate::scheduler::SCHEDULER;
use crate::shared_ref::SharedRef;
use crate::sink::EventEmitter;
use crate::sink::Sink;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;

pub enum Promise {
    SinkWait {
        sink: SharedRef<Sink>,
        buf: UserSlice,
    },
    SandboxedSyscall,
}

impl Promise {
    pub fn poll(&self, thread: &SharedRef<Thread>) -> Option<Result<usize, ErrorCode>> {
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
                    Err(error) => Some(Err(error)),
                }
            }
            Promise::SandboxedSyscall => None,
        }
    }
}

enum State {
    Created,
    Runnable,
    Blocked(Promise),
    Idle,
    Exited,
}

struct Mutable {
    state: State,
    emitter: Option<EventEmitter>,
    events: VecDeque<SyscallRegs>,
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
        let arch = if process.isolation().is_inkernel() {
            arch::Thread::new_kernel(entry, sp, start_info)
        } else {
            arch::Thread::new_user(entry, sp, start_info)
        };

        SharedRef::new(Self {
            arch,
            process,
            mutable: SpinLock::new(Mutable {
                state: State::Created,
                emitter: None,
                events: VecDeque::new(),
            }),
        })
    }

    pub fn new_idle() -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            arch: arch::Thread::new_idle(),
            process: IDLE_PROCESS.clone(),
            mutable: SpinLock::new(Mutable {
                state: State::Idle,
                emitter: None,
                events: VecDeque::new(),
            }),
        })
    }

    pub fn process(&self) -> &SharedRef<Process> {
        &self.process
    }

    pub fn block_on(&self, promise: Promise) {
        let mut mutable = self.mutable.lock();
        mutable.state = State::Blocked(promise);
    }

    pub fn block_on_sandboxed_syscall(&self, regs: SyscallRegs) {
        let mut mutable = self.mutable.lock();
        mutable.state = State::Blocked(Promise::SandboxedSyscall);
        mutable.events.push_back(regs);
        if let Some(emitter) = &mutable.emitter {
            emitter.notify();
        }
    }

    pub fn unblock(self: SharedRef<Self>) {
        // Evaluate the promsie again in Thread::poll.
        SCHEDULER.push(self);
    }

    pub fn start(self: &SharedRef<Self>) {
        let mut mutable = self.mutable.lock();
        debug_assert!(matches!(mutable.state, State::Created));
        mutable.state = State::Runnable;
        SCHEDULER.push(self.clone());
    }

    pub fn resume_with(self: &SharedRef<Self>, retval: usize) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        if !matches!(mutable.state, State::Blocked(Promise::SandboxedSyscall)) {
            return Err(ErrorCode::InvalidState);
        }

        // SAFETY: A blocked thread is not running.
        unsafe {
            self.do_set_syscall_result(retval);
        }

        mutable.state = State::Runnable;
        SCHEDULER.push(self.clone());
        Ok(())
    }

    pub fn exit(&self) {
        let mut mutable = self.mutable.lock();
        mutable.state = State::Exited;
    }

    /// Attempts to resolve the blocked state, and returns `true` if the
    /// thread is now runnable.
    pub fn poll(self: &SharedRef<Self>) -> bool {
        let mut mutable = self.mutable.lock();
        match &mutable.state {
            State::Runnable => true,
            State::Idle | State::Exited | State::Created => false,
            State::Blocked(promise) => {
                if let Some(result) = promise.poll(self) {
                    mutable.state = State::Runnable;
                    drop(mutable);
                    // SAFETY: The scheduler only polls non-running threads.
                    unsafe { self.set_syscall_result(result) };
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Sets the system call return value for this thread.
    ///
    /// # Safety
    ///
    /// This function must be called only when the thread is not running on
    /// another CPU. FIXME: Guarantee this!
    pub unsafe fn set_syscall_result(&self, retval: Result<usize, ErrorCode>) {
        // Encode the return value.
        let raw = match retval {
            Ok(retval) if retval >= ERROR_RETVAL_BASE => {
                trace!("invalid syscall return value: {:x}", retval);
                ERROR_RETVAL_BASE + ErrorCode::Unreachable as usize
            }
            Ok(retval) => retval,
            Err(error) => ERROR_RETVAL_BASE + error as usize,
        };

        unsafe {
            self.do_set_syscall_result(raw);
        }
    }

    unsafe fn do_set_syscall_result(&self, raw: usize) {
        // FIXME: Terrible hack
        let arch_thread = &self.arch as *const arch::Thread as *mut arch::Thread;
        unsafe {
            (*arch_thread).set_syscall_result(raw);
        }
    }
}

impl Handleable for Thread {
    fn set_event_emitter(&self, emitter: Option<EventEmitter>) -> Result<(), ErrorCode> {
        let mut mutable = self.mutable.lock();
        mutable.emitter = emitter;
        Ok(())
    }

    fn read_event(
        &self,
        handle_id: HandleId,
        _handle_table: &mut HandleTable,
    ) -> Result<Option<Event>, ErrorCode> {
        let mut mutable = self.mutable.lock();
        let Some(regs) = mutable.events.pop_front() else {
            return Ok(None);
        };

        Ok(Some(Event {
            sandboxed_syscall: SandboxedSyscallEvent {
                header: EventHeader {
                    ty: EventType::SANDBOXED_SYSCALL,
                    id: handle_id,
                },
                regs,
            },
        }))
    }
}

pub fn sys_thread_exit(current: &SharedRef<Thread>) -> Result<SyscallResult, ErrorCode> {
    current.exit();
    Ok(SyscallResult::Exit)
}

pub fn sys_thread_create(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
) -> Result<SyscallResult, ErrorCode> {
    let process_id = HandleId::from_raw(a0);
    let entry = a1;
    let sp = a2;
    let start_info = a3;

    let mut handle_table = current.process().handle_table().lock();
    let process = handle_table
        .get::<Process>(process_id)?
        .authorize(HandleRight::WRITE)?;

    let reserve = handle_table.reserve()?;

    let thread = Thread::new(process, entry, sp, start_info)?;
    let handle = Handle::new(thread, HandleRight::ALL);
    let id = reserve.insert(handle);
    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_thread_start(
    current: &SharedRef<Thread>,
    a0: usize,
) -> Result<SyscallResult, ErrorCode> {
    let thread_id = HandleId::from_raw(a0);

    current
        .process()
        .handle_table()
        .lock()
        .get::<Thread>(thread_id)?
        .authorize(HandleRight::WRITE)?
        .start();

    Ok(SyscallResult::Return(0))
}

pub fn sys_thread_resume_with(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let thread_id = HandleId::from_raw(a0);
    let retval = a1;

    current
        .process()
        .handle_table()
        .lock()
        .get::<Thread>(thread_id)?
        .authorize(HandleRight::WRITE)?
        .resume_with(retval)?;

    Ok(SyscallResult::Return(0))
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
    pub fn arch_thread(&self) -> *mut arch::Thread {
        static_assert!(offset_of!(Thread, arch) == 0);

        // SAFETY: The static_assert above guarantees arch::Thread is at the offset 0.
        unsafe { *self.ptr.get() as *mut arch::Thread }
    }
}

fn schedule(current: &CurrentThread) -> Option<*const arch::Thread> {
    let current_thread = current.thread();
    if matches!(current_thread.mutable.lock().state, State::Runnable) {
        // The current thread is runnable. Push it back to the scheduler.
        SCHEDULER.push_front(current_thread);
    }

    while let Some(thread) = SCHEDULER.pop() {
        if thread.poll() {
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
    let cpuvar = arch::get_cpuvar();
    let current = &cpuvar.current_thread;
    let Some(thread) = schedule(current) else {
        // Update the current thread. Otherwise, the interrupt handler would
        // overwrite the user's system call context (registers) with the idle
        // thread's context.
        current.update(cpuvar.idle_thread.clone());

        // No threads to run. Enter the idle loop.
        arch::idle();
    };

    // Switch the address space.
    // TODO: Should we cache the CR3 in the thread?
    current.thread().process().isolation().vmspace().switch();

    arch::thread_switch(thread);
}
