#![no_std]
#![no_main]

use ftl::application::Application;
use ftl::application::Context;
use ftl::println;

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
