#![no_std]
#![no_main]

use ftl::aio;
use ftl::channel::Channel;
use ftl::channel::OpenOptions;
use ftl::prelude::*;

#[ftl::main]
async fn main(supervisor_ch: Channel) {
    info!("starting ping");
    let supervisor_ch = aio::Client::new(supervisor_ch);
    let ch = supervisor_ch
        .open(b"service/pong", OpenOptions::CONNECT)
        .await
        .unwrap();
    let client = aio::Client::new(ch);

    info!("connected to pong");
    for _ in 0..10 {
        let written_len = client.write(b"Hello, world!").await.unwrap();
        info!("wrote {written_len} bytes");
    }

    info!("sent 10 messages");
}
