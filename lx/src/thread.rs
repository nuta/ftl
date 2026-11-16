use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::sync::Weak;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::mem::offset_of;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;

use ftl::poll::Poll;
use ftl::thread::Thread;
use ftl::trace;
use ftl::warn;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::thread::FaultFrame;
use ftl_types::thread::Regs;
use ftl_types::thread::RegsKind;
use ftl_utils::spinlock::SpinLock;

use crate::arch::FAULT_REGS_SIZE;
use crate::arch::SyscallFrame;
use crate::process::PId;
use crate::process::Process;
use crate::signal::SigDisposition;
use crate::signal::Signal;
use crate::signal::SignalSet;
use crate::types::c_int;
use crate::types::c_long;
use crate::types::errno::Errno;
use crate::types::signal::SIG_BLOCK;
use crate::types::signal::SIG_SETMASK;
use crate::types::signal::SIG_UNBLOCK;
use crate::types::sys::futex::FUTEX_BITSET_MATCH_ANY;
use crate::vm::Vm;

pub const SYSTEM_STACK_SIZE: usize = 256 * 1024;

// The system stack area, followed by the cookie. The kernel uses the top of the
// stack for the fault frame and the system call frame. It is aligned to 16
// bytes, as required by x86-64 ABI.
#[repr(C, align(16))]
struct SystemStack {
    stack: UnsafeCell<MaybeUninit<[u8; SYSTEM_STACK_SIZE]>>,
    fault_regs: UnsafeCell<MaybeUninit<[u8; FAULT_REGS_SIZE]>>,
    cookie: Cookie,
}

// SAFETY: The stack is used by a single thread, or is written by kernel's trap
//         handler when the thread is not running.
unsafe impl Sync for SystemStack {}

const SYSTEM_SP_OFFSET: usize = offset_of!(SystemStack, fault_regs);

struct Mutable {
    signal_frame: Option<SyscallFrame>,
    blocked: SignalSet,
    exit_status: Option<c_int>,
    clear_child_tid: Option<usize>,
}

pub struct LxThread {
    process: Weak<Process>,
    vm: Arc<Vm>,
    tid: PId,
    inner: Thread,
    mutable: SpinLock<Mutable>,
    system_stack: Box<SystemStack>,
    handling_fault: AtomicBool,
}

#[repr(align(16))]
struct Cookie {
    thread: Weak<LxThread>,
}

impl Drop for LxThread {
    fn drop(&mut self) {
        if let Err(err) = self.inner.destroy() {
            warn!("failed to destroy a thread: {:?}", err);
        }
    }
}

impl LxThread {
    pub fn new(vm: Arc<Vm>, process: Weak<Process>, tid: PId) -> Result<Arc<Self>, ErrorCode> {
        let mut system_stack =
            Box::<SystemStack>::try_new_uninit().map_err(|_| ErrorCode::OutOfMemory)?;
        let syscall_pc = crate::arch::syscall_handler as *const () as usize;
        let fault_pc = crate::arch::fault_handler as *const () as usize;
        let system_sp = system_stack.as_ptr().addr() + SYSTEM_SP_OFFSET;

        // TODO: LX assumes that kernel won't touch the system stack until the
        //       thread is started. Should we document and guarantee this?
        let inner = Thread::create(vm.quota(), vm.vmspace(), syscall_pc, fault_pc, system_sp)?;

        let thread = Arc::new_cyclic(|thread| {
            let cookie = Cookie {
                thread: thread.clone(),
            };
            unsafe { (&raw mut (*system_stack.as_mut_ptr()).cookie).write(cookie) };

            LxThread {
                process,
                vm,
                tid,
                inner,
                mutable: SpinLock::new(Mutable {
                    signal_frame: None,
                    blocked: SignalSet::empty(),
                    exit_status: None,
                    clear_child_tid: None,
                }),
                system_stack: unsafe { system_stack.assume_init() },
                handling_fault: AtomicBool::new(false),
            }
        });

        Ok(thread)
    }

    pub fn enter_fault_handler(&self) -> bool {
        self.handling_fault.swap(true, Ordering::Relaxed)
    }

    pub fn leave_fault_handler(&self) {
        let prev = self.handling_fault.swap(false, Ordering::Relaxed);
        debug_assert!(prev);
    }

    pub fn start(&self, entry: usize, sp: usize) -> Result<(), ErrorCode> {
        self.inner.start(entry, sp)
    }

    pub fn push_syscall_frame(&self, frame: SyscallFrame) -> usize {
        let stack: *mut u8 = self.system_stack.stack.get().cast();
        unsafe {
            let sp = stack
                .add(SYSTEM_STACK_SIZE - size_of::<FaultFrame>() - size_of::<SyscallFrame>())
                .cast::<SyscallFrame>();
            sp.write(frame);
            sp.addr()
        }
    }

    pub fn id(&self) -> HandleId {
        self.inner.id()
    }

    pub fn subscribe(&self, poll: &Poll, cookie: usize) -> Result<(), ErrorCode> {
        self.inner.subscribe(poll, cookie)
    }

    pub fn tid(&self) -> PId {
        self.tid
    }

    pub fn process(&self) -> Arc<Process> {
        self.process.upgrade().unwrap()
    }

    /// Cleans up an exited thread.
    ///
    /// Note: Call this in LX's main loop.
    pub fn reap(&self) -> Result<(), Errno> {
        let Some(process) = self.process.upgrade() else {
            return Ok(());
        };

        // Clear the TID, and wake up the waiter.
        let clear_child_tid = self.mutable.lock().clear_child_tid;
        if let Some(uaddr) = clear_child_tid {
            if let Err(err) = self.clear_child_tid(&process, uaddr) {
                trace!("failed to clear child TID at {:#x}: {:?}", uaddr, err);
            }
        }

        process.on_thread_exit(self)
    }

    /// Note: Call this in LX's main loop.
    fn clear_child_tid(&self, process: &Process, uaddr: usize) -> Result<(), Errno> {
        // Don't access uaddr directly. It is in a different address space
        // since we're in the LX's main loop.
        self.vm.write(uaddr, &0u32.to_ne_bytes())?;
        process.futexes().wake(uaddr, 1, FUTEX_BITSET_MATCH_ANY)?;
        Ok(())
    }

    pub fn set_clear_child_tid(&self, uaddr: Option<usize>) {
        self.mutable.lock().clear_child_tid = uaddr;
    }

    pub fn exit_status(&self) -> Option<c_int> {
        self.mutable.lock().exit_status
    }

    pub fn set_exit_status(&self, status: c_int) {
        self.mutable.lock().exit_status = Some(status);
    }

    pub fn vm(&self) -> &Arc<Vm> {
        &self.vm
    }

    pub fn set_fsbase(&self, fsbase: usize) -> Result<(), ErrorCode> {
        self.inner
            .write_regs(RegsKind::FsBase, Regs { fs_base: fsbase })
    }

    pub fn copy_regs_to(&self, dest: &LxThread, kind: RegsKind) -> Result<(), ErrorCode> {
        self.inner.copy_regs_to(&dest.inner, kind)
    }

    pub fn signal_mask(&self) -> SignalSet {
        self.mutable.lock().blocked
    }

    pub fn set_signal_mask(&self, mask: SignalSet) {
        let mut mask = mask;

        // SIGKILL/SIGSTOP signals cannot be blocked.
        mask.remove(Signal::KILL);
        mask.remove(Signal::STOP);

        self.mutable.lock().blocked = mask;
    }

    pub fn sigprocmask(&self, how: c_int, set: Option<SignalSet>) -> Result<SignalSet, Errno> {
        let old = self.signal_mask();
        let Some(set) = set else {
            return Ok(old);
        };

        let new = match how {
            SIG_BLOCK => old.union(set),
            SIG_UNBLOCK => old.difference(set),
            SIG_SETMASK => set,
            _ => return Err(Errno::EINVAL),
        };

        self.set_signal_mask(new);
        Ok(old)
    }

    /// Modifies the thread's state to return to the signal handler.
    pub fn handle_pending_signal(&self, frame: &mut SyscallFrame) {
        if self.mutable.lock().signal_frame.is_some() {
            // A signal is already being delivered.
            return;
        }

        let process = self.process();
        let (signal, action, handler) = loop {
            let Some((signal, action)) = process.take_pending_signal(self.signal_mask()) else {
                // No pending signal to deliver.
                return;
            };

            match action.disposition() {
                SigDisposition::Handler(handler) => break (signal, action, handler),
                SigDisposition::Ignore => {
                    // Handle the next pending signal.
                    continue;
                }
                SigDisposition::Default => {
                    trace!(
                        "default signal handling for {:?} is not implemented",
                        signal
                    );

                    // Handle the next pending signal.
                    continue;
                }
            }
        };

        // Save the original system call frame. We'll restore it when returning
        // from the signal handler.
        self.mutable.lock().signal_frame = Some(*frame);

        unsafe {
            frame.enter_signal(signal.number() as usize, handler, action.restorer());
        }
    }

    /// Restores the thread's state to resume from signal handling.
    pub fn return_from_signal(&self, frame: &mut SyscallFrame) -> Result<c_long, Errno> {
        let mut mutable = self.mutable.lock();
        let saved = mutable.signal_frame.take().ok_or(Errno::EINVAL)?;
        *frame = saved;
        Ok(saved.retval())
    }

    /// # Safety
    ///
    /// `system_sp` must be the thread's system stack pointer which we created
    /// in [`LxThread::new`]. Also, the caller must ensure that the thread is
    /// not freed.
    pub unsafe fn from_system_sp<'a>(system_sp: usize) -> &'a LxThread {
        let stack = (system_sp - SYSTEM_SP_OFFSET) as *const SystemStack;
        let cookie = unsafe { &(*stack).cookie };
        unsafe { &*cookie.thread.as_ptr() }
    }
}
