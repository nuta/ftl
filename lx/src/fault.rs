use alloc::sync::Arc;

use ftl::trace;
use ftl::warn;
use ftl_types::thread::ExitReason;
use ftl_types::thread::Fault;
use ftl_types::thread::PageFaultInfo;

use crate::arch::FaultFrame;
use crate::process::Process;
use crate::thread::LxThread;

enum FaultResult {
    Resolved,
    Exit,
}

fn do_handle_fault(
    frame: &mut FaultFrame,
    current: &LxThread,
    process: &Arc<Process>,
) -> FaultResult {
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
                    trace!("failed to handle page fault at {:#x}: {:?}", addr, errno);
                    FaultResult::Exit
                }
            }
        }
        _ => {
            warn!(
                "pid {}: killed by {:?}: pc={:#x}, addr={:#x}, info={:#x}",
                process.id().as_int(),
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
    let frame = unsafe { &mut *frame };
    let current = unsafe { LxThread::from_cookie(frame.cookie) };
    let process = current.process();

    match do_handle_fault(frame, current, &process) {
        FaultResult::Resolved => frame,
        FaultResult::Exit => {
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
