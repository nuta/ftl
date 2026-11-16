use ftl::trace;
use ftl::warn;
use ftl_types::thread::ExitReason;
use ftl_types::thread::Fault;
use ftl_types::thread::PageFaultInfo;

use crate::arch::FaultFrame;
use crate::thread::LxThread;

enum FaultResult {
    Resolved,
    Exit,
}

fn do_handle_fault(frame: &mut FaultFrame, current: &LxThread) -> FaultResult {
    let fault = frame.fault;
    let rip = frame.rip;
    let addr = frame.addr;
    let info = frame.info;

    match fault {
        Fault::PageFault => {
            let info = PageFaultInfo::from_raw(info);
            match current.vm().handle_page_fault(addr, info) {
                Ok(()) => FaultResult::Resolved,
                Err(errno) => {
                    // FIXME: If the fault happened in LX (e.g. a system call
                    //        handler dereferences a bad user pointer), we
                    //        should return EFAULT instead of killing the
                    //        process.
                    //
                    //        What's worse, exiting here may leaks locks and
                    //        cause dead locks.
                    trace!("failed to handle page fault at {:#x}: {:?}", addr, errno);
                    FaultResult::Exit
                }
            }
        }
        _ => {
            warn!(
                "pid {}: killed by {:?}: pc={:#x}, addr={:#x}, info={:#x}",
                current.process().id().as_int(),
                fault,
                rip,
                addr,
                info
            );
            FaultResult::Exit
        }
    }
}

pub extern "C" fn handle_fault(frame: *mut FaultFrame) -> *mut FaultFrame {
    // Note: ftl::thread::exit() never returns. Avoid keeping Drop guards in
    //       this function.
    let system_sp = frame.addr() + size_of::<ftl_types::thread::FaultFrame>();
    let current = unsafe { LxThread::from_system_sp(system_sp) };
    let frame = unsafe { &mut *frame };

    if current.enter_fault_handler() {
        panic!(
            "nested fault: {:?}: pc={:#x}, addr={:#x}",
            frame.fault, frame.rip, frame.addr
        );
    }

    match do_handle_fault(frame, current) {
        FaultResult::Resolved => {
            current.leave_fault_handler();
            frame
        }
        FaultResult::Exit => {
            let process = current.process();

            // FIXME: Propagate the proper signal, not 1.
            if let Err(errno) = process.exit(1) {
                warn!("failed to exit the process: {:?}", errno);
            }

            // ftl::thread::exit() never returns. Drop guards before calling it.
            drop(process);
            ftl::thread::exit(ExitReason::Errored)
        }
    }
}
