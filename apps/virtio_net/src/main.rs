#![no_std]
#![no_main]

use ftl::application::Context;
use ftl::application::OpenRequest;
use ftl::application::ReadRequest;
use ftl::channel::Channel;
use ftl::error::ErrorCode;
use ftl::println;

struct Main {}

impl ftl::application::Application for Main {
    fn init() -> Self {
        Self {}
    }

    fn open(&mut self, ctx: &mut Context<Channel>, req: OpenRequest) {
        let (our_ch, their_ch) = Channel::new()?;
        ctx.add(our_ch)?;
        req.complete(their_ch);
    }

    fn read(&mut self, ctx: &mut Context<Channel>, req: ReadRequest) {
        // Pop a packet from the virtio queue.
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::main::<Main>();
}
