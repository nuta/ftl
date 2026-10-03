use ftl::warn;
use ftl_types::thread::ExitReason;

use crate::arch::FaultFrame;
use crate::thread::LxThread;

enum FaultResult {
    #[allow(dead_code)]
    Resolved,
    Exit,
}

fn do_handle_fault(frame: *mut FaultFrame) -> FaultResult {
    let frame = unsafe { &mut *frame };
    let current = unsafe { LxThread::from_cookie(frame.cookie) };
    let fault = frame.fault;
    let rip = frame.rip;
    let addr = frame.addr;
    let info = frame.info;

    let process = current.process();
    warn!(
        "pid {}: killed by {:?}: pc={:#x}, addr={:#x}, info={:#x}",
        process.id().as_int(),
        fault,
        rip,
        addr,
        info
    );

    // FIXME: Propagate the proper signal, not 1.
    if let Err(errno) = process.exit(1) {
        warn!("failed to exit the process: {:?}", errno);
    }

    FaultResult::Exit
}

pub extern "C" fn handle_fault(frame: *mut FaultFrame) -> *mut FaultFrame {
    // Note: ftl::thread::exit() never returns. Avoid keeping Drop guards in
    //       this function.
    match do_handle_fault(frame) {
        FaultResult::Resolved => frame,
        FaultResult::Exit => ftl::thread::exit(ExitReason::Errored),
    }
}
