#![no_std]
#![no_main]

use ftl_api::channel::Channel;
use ftl_api::handle::OwnedHandle;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::types::message::MessageBuffer;
use ftl_api_autogen::apps::http_server::Environ;
use ftl_api_autogen::apps::http_server::Message;

#[derive(Debug)]
enum Context {
    Autopilot,
    Tcpip,
    TcpSock,
}

#[ftl_api::main]
pub fn main(mut env: Environ) {
    info!("starting");
    let tcpip_ch = env.depends.tcpip.take().unwrap();
    let mut buffer = MessageBuffer::new();
    let mut mainloop = Mainloop::<Context, Message>::new().unwrap();
    mainloop
        .add_channel(env.autopilot_ch.take().unwrap(), Context::Autopilot)
        .unwrap();
    mainloop
        .add_channel(tcpip_ch, Context::Tcpip)
        .unwrap();

    loop {
        trace!("waiting for event...");
        match mainloop.next(&mut buffer) {
            Event::Message { ctx, ch, m } => {
                match (ctx, m) {
                    (Context::Tcpip, Message::TcpAccepted(m)) => {
                        let ch = Channel::from_handle(OwnedHandle::from_raw(m.sock()));
                        mainloop.add_channel(ch, Context::TcpSock).unwrap();
                    }
                    (Context::TcpSock, Message::TcpReceived(m)) => {
                        m.data().as_slice();
                    }
                    (_, m) => {
                        warn!("unexpected message: {:?}", m);
                    }
                }
            }
            ev => {
                warn!("unexpected event: {:?}", ev);
            }
        }
    }
}
