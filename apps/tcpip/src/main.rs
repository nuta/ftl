#![no_std]
#![no_main]

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::OpenCompleter;
use ftl::application::ReadCompleter;
use ftl::application::WriteCompleter;
use ftl::collections::VecDeque;
use ftl::println;
use smoltcp::iface::SocketHandle;

enum State {
    TcpConn {
        socket: SocketHandle,
        pending_reads: VecDeque<ReadCompleter>,
        pending_writes: VecDeque<WriteCompleter>,
    },
    TcpListener {
        socket: SocketHandle,
        pending_accepts: VecDeque<OpenCompleter>,
    },
}

struct Main {}

impl Application for Main {
    fn init(ctx: &mut Context) -> Self {
        Self {}
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
