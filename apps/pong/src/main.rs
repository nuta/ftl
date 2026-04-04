#![no_std]
#![no_main]

use ftl::aio;
use ftl::aio::Request;
use ftl::channel::Channel;
use ftl::channel::OpenOptions;
use ftl::error::ErrorCode;
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
            Request::Open {
                path,
                options: _,
                completer,
            } => {
                let mut buf = vec![0; path.len()];
                path.read_all(&mut buf).unwrap();
                info!("received open message: {:?}", core::str::from_utf8(&buf));

                let (ours, theirs) = Channel::new().unwrap();
                completer.reply(theirs).unwrap();
                ours
            }
            _ => {
                warn!("unhandled request: {:?}", req);
                continue;
            }
        };

        aio::spawn(async move {
            info!("server spawned");
            let server = aio::Server::new(client_ch);
            loop {
                match server.recv().await {
                    Ok(Request::Write {
                        offset: _,
                        data,
                        completer,
                    }) => {
                        let data_len = data.len();
                        let mut buf = vec![0; data_len];
                        data.read_all(&mut buf).unwrap();
                        info!("received write message: {:?}", core::str::from_utf8(&buf));
                        completer.reply(data_len).unwrap();
                    }
                    Err(ErrorCode::PeerClosed) => {
                        debug!("peer closed");
                        break;
                    }
                    result => {
                        warn!("unhandled recv: {:?}", result);
                    }
                }
            }
        });
    }
}
