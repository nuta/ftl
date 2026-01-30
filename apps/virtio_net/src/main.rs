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
        let (our_ch, their_ch) = match Channel::new() {
            Ok((our_ch, their_ch)) => (our_ch, their_ch),
            Err(error) => {
                req.error(error);
                return;
            }
        };

        if let Err(err) = ctx.add(our_ch) {
            println!("failed to add our channel: {:?}", err);
            req.error(ErrorCode::Unreachable);
        }

        req.complete(their_ch);
    }

    fn read(&mut self, ctx: &mut Context<Channel>, req: ReadRequest) {
        // Pop a packet from the virtio queue.
    }
}

fn main() {
    ftl::application::main::<Main>();
}
