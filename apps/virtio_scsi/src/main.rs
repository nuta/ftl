#![no_std]
#![no_main]

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::InitContext;
use ftl::prelude::*;
use ftl_virtio::VirtioPci;
use ftl_virtio::virtio_pci::DeviceType;

struct Main {
    virtio: VirtioPci,
}

impl Application for Main {
    fn init(ctx: &mut InitContext) -> Self {
        trace!("starting...");
        let prober = VirtioPci::probe(DeviceType::Scsi).unwrap();
        let device_features = prober.read_guest_features();
        let guest_features = device_features;
        let (virtio, interrupt) = prober.finish(guest_features);

        ctx.add_interrupt(interrupt).unwrap();
        Self { virtio }
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
