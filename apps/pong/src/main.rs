#![no_std]
#![no_main]

use ftl::aio;
use ftl::aio::Request;
use ftl::channel::Channel;
use ftl::channel::OpenOptions;
use ftl::prelude::*;

#[ftl::main]
async fn main(supervisor_ch: Channel) {
    info!("starting pong");
    let supervisor_ch = aio::Client::new(supervisor_ch);
    let listen_ch = supervisor_ch
        .open(b"service/pong", OpenOptions::LISTEN)
        .await
        .unwrap();

    let listen_ch = aio::Server::new(listen_ch);
    loop {
        info!("waiting for client");
        let req = listen_ch.recv().await.unwrap();
        let client_ch = match req {
            Request::Open { path, options } => {
                info!("received open message");
                let mut buf = vec![0; path.len()];
                path.read_all(&mut buf).unwrap();
                info!("path: {:?}", core::str::from_utf8(&buf));
                let (ours, theirs) = Channel::new().unwrap();
                // TODO: reply here
                ours
            }
            _ => {
                warn!("unhandled request: {:?}", req);
                continue;
            }
        };

        let server = aio::Server::new(client_ch);
        info!("server created");
        aio::spawn(async move {
            info!("server spawned");
            loop {
                match server.recv().await {
                    Ok(Request::Write { offset: _, data }) => {
                        let mut buf = vec![0; data.len()];
                        data.read_all(&mut buf).unwrap();
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
