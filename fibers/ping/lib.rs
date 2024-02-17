#![no_std]

use ftl_api::environ::Environ;
use ftl_api::println;
use ftl_api::Message;
use ftl_autogen::fibers::ping::Deps;

pub fn main(mut env: Environ) {
    let mut deps = env.parse_deps::<Deps>().expect("failed to parse deps");

    println!("fiber 1: hello");
    for i in 0.. {
        deps.pong.send(Message::Ping(i)).unwrap();
        let msg = deps.pong.receive().unwrap();
        println!("filber1: received {:?}", msg);
    }
}
