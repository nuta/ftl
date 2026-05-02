#![no_std]
#![no_main]

use core::ops::ControlFlow;

use ftl::aio;
use ftl::aio::Client;
use ftl::channel::Channel;
use ftl::channel::OpenOptions;
use ftl::prelude::*;

use crate::connection::Connection;

mod connection;

const RECV_BUFFER_SIZE: usize = 4096;

async fn handle_connection(conn_ch: Client) {
    let mut conn = Connection::new();
    let mut recv_buf = vec![0; RECV_BUFFER_SIZE];

    // Read a request.
    loop {
        let data = match conn_ch.read(0, &mut recv_buf).await {
            Ok(data) => data,
            Err(error) => {
                if error == ftl::error::ErrorCode::PeerClosed {
                    trace!("connection peer closed");
                } else {
                    warn!("failed to read from connection: {:?}", error);
                }
                return;
            }
        };

        trace!("received {} bytes", data.len());
        if matches!(conn.handle_recv(data), ControlFlow::Break(())) {
            break;
        }
    }

    // Send a response.
    while let Some(data) = conn.poll_send() {
        let mut written = 0;
        while written < data.len() {
            let len = match conn_ch.write(0, &data[written..]).await {
                Ok(0) => {
                    warn!("failed to write response: zero-length write");
                    return;
                }
                Ok(len) => len,
                Err(error) => {
                    warn!("failed to write response: {:?}", error);
                    return;
                }
            };

            trace!("wrote {} bytes", len);
            written += len;
        }
    }
}

#[ftl::main]
fn main(supervisor_ch: Channel) {
    info!("starting http_server");

    aio::run(async move {
        let supervisor = Client::new(supervisor_ch);
        let tcpip = match supervisor
            .open(b"service/tcpip", OpenOptions::CONNECT)
            .await
        {
            Ok(handle) => Client::new(Channel::from_handle(handle)),
            Err(error) => panic!("failed to open tcpip service: {:?}", error),
        };
        let listener = match tcpip
            .open(b"tcp-listen:0.0.0.0:80", OpenOptions::LISTEN)
            .await
        {
            Ok(handle) => Client::new(Channel::from_handle(handle)),
            Err(error) => panic!("failed to open tcp listener: {:?}", error),
        };

        trace!("listening on 80");
        loop {
            let conn_ch = match listener.open(b"", OpenOptions::CONNECT).await {
                Ok(handle) => Client::new(Channel::from_handle(handle)),
                Err(error) => {
                    warn!("failed to accept connection: {:?}", error);
                    continue;
                }
            };

            trace!("accepted a connection");
            aio::spawn(async move {
                handle_connection(conn_ch).await;
            });
        }
    });
}
