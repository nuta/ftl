use ftl::trace;
use ftl::warn;
use ftl_types::thread::ExitReason;
use ftl_types::thread::Fault;
use ftl_types::thread::PageFaultInfo;

use crate::arch::FaultFrame;
use crate::arch::UserContext;
use crate::signal::Signal;
use crate::thread::SYSTEM_STACK_SIZE;
use crate::thread::Thread;
use crate::types::signal::BUS_ADRALN;
use crate::types::signal::FPE_INTDIV;
use crate::types::signal::ILL_ILLOPN;
use crate::types::signal::SEGV_ACCERR;
use crate::types::signal::SEGV_MAPERR;
use crate::types::signal::SI_KERNEL;
use crate::types::signal::SigInfo;
use crate::types::signal::TRAP_BRKPT;

enum FaultResult {
    Resolved,
    Signal { signal: Signal, siginfo: SigInfo },
    Exit,
}

fn do_handle_fault(frame: &mut FaultFrame, current: &Thread, in_system: bool) -> FaultResult {
    let fault = frame.fault;
    let rip = frame.rip;
    let addr = frame.addr;
    let info = frame.info;

    if fault == Fault::PageFault {
        let info = PageFaultInfo::from_raw(info);
        match current.vm().handle_page_fault(addr, info) {
            Ok(()) => return FaultResult::Resolved,
            Err(errno) if in_system => {
                // FIXME: If the fault happened in the system (e.g. a system call
                //        handler dereferences a bad user pointer), we
                //        should return EFAULT instead of killing the
                //        process.
                //
                //        What's worse, exiting here may leaks locks and
                //        cause dead locks.
                trace!("failed to handle page fault at {:#x}: {:?}", addr, errno);
                return FaultResult::Exit;
            }
            Err(_) => {}
        }
    }

    if in_system {
        warn!(
            "pid {}: {:?} in the system: pc={:#x}, addr={:#x}, info={:#x}",
            current.process().id().as_int(),
            fault,
            rip,
            addr,
            info
        );
        return FaultResult::Exit;
    }

    let (signal, code, addr) = match fault {
        Fault::PageFault => {
            let code = if current.vm().is_mapped(addr) {
                SEGV_ACCERR
            } else {
                SEGV_MAPERR
            };

            (Signal::SEGV, code, addr)
        }
        Fault::GeneralProtectionFault => (Signal::SEGV, SI_KERNEL, 0),
        Fault::StackSegmentFault => (Signal::BUS, SI_KERNEL, 0),
        Fault::DivideError => (Signal::FPE, FPE_INTDIV, rip),
        Fault::FloatingPointError | Fault::SimdFloatingPoint => (Signal::FPE, 0, rip),
        Fault::InvalidOpcode => (Signal::ILL, ILL_ILLOPN, rip),
        Fault::Debug => (Signal::TRAP, TRAP_BRKPT, rip),
        Fault::Breakpoint => (Signal::TRAP, SI_KERNEL, 0),
        Fault::AlignmentCheck => (Signal::BUS, BUS_ADRALN, 0),
    };

    FaultResult::Signal {
        signal,
        siginfo: SigInfo::new(signal.number(), code, addr),
    }
}

pub extern "C" fn handle_fault(frame: *mut FaultFrame) -> *mut FaultFrame {
    // Note: ftl::thread::exit() never returns. Avoid keeping Drop guards in
    //       this function.
    let system_sp = frame.addr() + size_of::<ftl_types::thread::FaultFrame>();
    let current = unsafe { Thread::from_system_sp(system_sp) };
    let frame = unsafe { &mut *frame };

    if current.enter_fault_handler() {
        panic!(
            "nested fault: {:?}: pc={:#x}, addr={:#x}",
            frame.fault, frame.rip, frame.addr
        );
    }

    let in_system = system_sp.wrapping_sub(frame.rsp) < SYSTEM_STACK_SIZE;
    match do_handle_fault(frame, current, in_system) {
        FaultResult::Resolved => {
            current.leave_fault_handler();
            return frame;
        }
        FaultResult::Signal { signal, siginfo } => {
            let mut ctx = UserContext::from_fault_frame(frame);
            current.leave_fault_handler();
            if current.force_signal(&mut ctx, signal, siginfo) {
                ctx.restore();
            }

            warn!(
                "pid {}: killed by {:?}: pc={:#x}, addr={:#x}",
                current.process().id().as_int(),
                signal,
                frame.rip,
                siginfo.addr,
            );
        }
        FaultResult::Exit => {}
    }

    let process = current.process();

    // FIXME: Propagate the proper signal, not 1.
    if let Err(errno) = process.exit(1) {
        warn!("failed to exit the process: {:?}", errno);
    }

    // ftl::thread::exit() never returns. Drop guards before calling it.
    drop(process);
    ftl::thread::exit(ExitReason::Errored)
}
