#![no_std]
#![no_main]

use ftl::application::Context;
use ftl::application::OpenRequest;
use ftl::application::ReadRequest;
use ftl::channel::Channel;
use ftl::error::ErrorCode;
use ftl::println;

mod virtio;

#[derive(serde::Deserialize)]
struct Env {
    bus: u8,
    slot: u8,
    iobase: u16,
}

struct Main {
    virtio: virtio::VirtioPci,
}

impl ftl::application::Application<Env> for Main {
    fn init(env: Env) -> Self {
        let virtio = virtio::VirtioPci::new(env.bus, env.slot, env.iobase);
        Self { virtio }
    }

    fn open(&mut self, ctx: &mut Context<Channel>, req: OpenRequest) {
        let (our_ch, their_ch) = match Channel::new() {
            Ok(pair) => pair,
            Err(error) => {
                req.error(error);
                return;
            }
        };

        if let Err(err) = ctx.add(our_ch) {
            req.error(ErrorCode::RetryLater);
            return;
        }

        req.complete(their_ch);
    }

    fn read(&mut self, ctx: &mut Context<Channel>, req: ReadRequest) {
        // Pop a packet from the virtio queue.
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::main::<Main, Env>();
}
