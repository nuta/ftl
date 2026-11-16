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
use crate::arch::UserContext;
use crate::process::PId;
use crate::process::Process;
use crate::signal::SigAction;
use crate::signal::SigDisposition;
use crate::signal::Signal;
use crate::signal::SignalSet;
use crate::types::c_int;
use crate::types::errno::Errno;
use crate::types::signal::MINSIGSTKSZ;
use crate::types::signal::SA_NODEFER;
use crate::types::signal::SA_ONSTACK;
use crate::types::signal::SI_USER;
use crate::types::signal::SIG_BLOCK;
use crate::types::signal::SIG_SETMASK;
use crate::types::signal::SIG_UNBLOCK;
use crate::types::signal::SS_AUTODISARM;
use crate::types::signal::SS_DISABLE;
use crate::types::signal::SS_ONSTACK;
use crate::types::signal::SigInfo;
use crate::types::signal::StackT;
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

#[derive(Clone, Copy)]
struct AltStack {
    sp: usize,
    size: usize,
}

impl AltStack {
    fn contains(&self, sp: usize) -> bool {
        self.sp <= sp && sp - self.sp < self.size
    }
}

struct Mutable {
    blocked: SignalSet,
    pending_signals: SignalSet,
    altstack: Option<AltStack>,
    exit_status: Option<c_int>,
    clear_child_tid: Option<usize>,
}

impl Mutable {
    fn altstack_info(&self, current_sp: usize) -> StackT {
        match self.altstack {
            Some(altstack) => {
                StackT {
                    sp: altstack.sp,
                    flags: if altstack.contains(current_sp) {
                        SS_ONSTACK
                    } else {
                        0
                    },
                    size: altstack.size,
                }
            }
            None => {
                StackT {
                    sp: 0,
                    flags: SS_DISABLE,
                    size: 0,
                }
            }
        }
    }
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
                    blocked: SignalSet::empty(),
                    pending_signals: SignalSet::empty(),
                    altstack: None,
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

    pub fn sigaltstack(&self, new: Option<StackT>, current_sp: usize) -> Result<StackT, Errno> {
        let mut mutable = self.mutable.lock();

        let old = mutable.altstack_info(current_sp);

        let Some(new) = new else {
            // No new stack was specified. They just want to query the current
            // stack.
            return Ok(old);
        };

        if old.flags == SS_ONSTACK {
            // The user thread currently using the alternate stack.
            return Err(Errno::EPERM);
        }

        match new.flags & !SS_AUTODISARM {
            SS_DISABLE => {
                mutable.altstack = None;
            }
            0 | SS_ONSTACK => {
                if new.size < MINSIGSTKSZ {
                    return Err(Errno::ENOMEM);
                }

                mutable.altstack = Some(AltStack {
                    sp: new.sp,
                    size: new.size,
                });
            }
            _ => {
                return Err(Errno::EINVAL);
            }
        }

        Ok(old)
    }

    pub fn queue_signal(&self, signal: Signal) -> Result<(), Errno> {
        let process = self.process();
        match process.get_signal_action(signal).disposition() {
            SigDisposition::Ignore => return Ok(()),
            SigDisposition::Default => {
                trace!("unsupported default handling for signal {:?}", signal);
                return Err(Errno::ENOTSUP);
            }
            SigDisposition::Handler(_) => {}
        }

        self.mutable.lock().pending_signals.insert(signal);
        process.signal_wait_queue().notify(None);
        Ok(())
    }

    pub fn has_pending_signal(&self) -> bool {
        let mutable = self.mutable.lock();
        let blocked = mutable.blocked;
        if !mutable.pending_signals.difference(blocked).is_empty() {
            return true;
        }

        drop(mutable);
        self.process().has_unblocked_signal(blocked)
    }

    fn take_pending_signal(&self) -> Option<(Signal, SigAction)> {
        let process = self.process();
        let mut mutable = self.mutable.lock();
        let blocked = mutable.blocked;

        // Get a pending signal delivered to this thread.
        if let Some(signal) = mutable.pending_signals.difference(blocked).take_first() {
            mutable.pending_signals.remove(signal);
            drop(mutable);
            return Some((signal, process.get_signal_action(signal)));
        }

        drop(mutable);

        // Get a pending signal delivered to the process.
        process.take_pending_signal(blocked)
    }

    /// Modifies `ctx` to call the handler of a pending signal, if any.
    pub fn handle_pending_signal(&self, ctx: &mut UserContext) {
        loop {
            let Some((signal, action)) = self.take_pending_signal() else {
                // No pending signal to deliver.
                return;
            };

            match action.disposition() {
                SigDisposition::Handler(handler) => {
                    let info = SigInfo::new(signal.number(), SI_USER, 0);
                    self.enter_signal(ctx, signal, action, handler, info);
                    return;
                }
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
        }
    }

    fn enter_signal(
        &self,
        ctx: &mut UserContext,
        signal: Signal,
        action: SigAction,
        handler: usize,
        info: SigInfo,
    ) {
        let mutable = self.mutable.lock();
        let sigmask = mutable.blocked;

        let mut sp = ctx.sp();
        let stack = mutable.altstack_info(sp);
        if action.flags() & SA_ONSTACK != 0
            && let Some(altstack) = mutable.altstack
            && !altstack.contains(sp)
        {
            sp = altstack.sp + altstack.size;
        }

        drop(mutable);

        let mut blocked = sigmask.union(action.mask());
        if action.flags() & SA_NODEFER == 0 {
            blocked.insert(signal);
        }
        self.set_signal_mask(blocked);

        unsafe {
            ctx.enter_signal(sp, handler, action.restorer(), info, stack, sigmask);
        }
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
