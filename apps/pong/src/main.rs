#![no_std]
#![no_main]

use ftl::aio;
use ftl::aio::Request;
use ftl::channel::Channel;
use ftl::channel::OpenOptions;
use ftl::prelude::*;

async fn async_main(supervisor_ch: Channel) {
    info!("starting pong");
    let supervisor_ch = aio::Client::new(supervisor_ch);
    let listen_ch = supervisor_ch
        .open(b"service/pong", OpenOptions::LISTEN)
        .await
        .unwrap();
    let listen_client = aio::Client::new(listen_ch);

    loop {
        let client_ch = listen_client
            .open(b"*", OpenOptions::CONNECT)
            .await
            .unwrap();

        let server = aio::Server::new(client_ch);
        aio::spawn(async move {
            loop {
                match server.recv().await {
                    Ok(Request::Write { offset: _, data }) => {
                        let mut buf = vec![0; data.len()];
                        data.read(&mut buf).unwrap();
                        info!("received write message: {:?}", core::str::from_utf8(&buf));
                        // client_ch.send_args(MessageKind::WRITE_REPLY, 0, data.len(), 0).await.unwrap();
                    }
                    _ => {
                        todo!();
                    }
                }
            }
        });
    }
}

#[ftl::main]
fn main(supervisor_ch: Channel) {
    aio::run(async {
        async_main(supervisor_ch).await;
    });
}
