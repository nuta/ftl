#![no_std]

use ftl_api::environ::Environ;
use ftl_api::println;
use ftl_api::Message;
use ftl_autogen::fibers::pong::Deps;

pub fn main(mut env: Environ) {
    let mut deps = env.parse_deps::<Deps>().expect("failed to parse deps");

    println!("fiber 2: world");
    for i in 0.. {
        let msg = deps.ping.receive().unwrap();
        println!("filber2: received {:?}", msg);
        deps.ping.send(Message::Pong(7000000 + i)).unwrap();
    }
}
