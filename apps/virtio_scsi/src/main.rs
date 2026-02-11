#![no_std]
#![no_main]

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::InitContext;
use ftl::prelude::*;

struct Main {}

impl Application for Main {
    fn init(ctx: &mut InitContext) -> Self {
        trace!("starting...");
        Self {}
    }
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
