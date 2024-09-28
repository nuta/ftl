#![no_std]
#![no_main]

ftl_api::autogen!();

use device::NetDevice;
use ftl_api::channel::Channel;
use ftl_api::environ::Environ;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_autogen::idl::ethernet_device;
use ftl_autogen::idl::tcpip::TcpListenReply;
use ftl_autogen::idl::Message;
use smoltcp::iface::SocketHandle;
use smoltcp::wire::EthernetAddress;
use smoltcp::wire::HardwareAddress;
use tcpip::TcpIp;

mod device;
mod smotcp_log;
mod tcpip;

#[derive(Debug)]
enum Context {
    Startup,
    Driver,
    CtrlSocket,
    DataSocket(SocketHandle),
}

#[no_mangle]
pub fn main(mut env: Environ) {
    info!("starting");
    let driver_ch = env.take_channel("dep:ethernet_device").unwrap();
    let startup_ch = env.take_channel("dep:startup").unwrap();

    let mac = HardwareAddress::Ethernet(EthernetAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56])); // FIXME:

    // The ethernet device will call this closure to transmit packets.
    let (driver_sender, driver_receiver) = driver_ch.split();
    let transmit = {
        let driver_sender = driver_sender.clone();
        move |buf: &[u8]| {
            trace!("transmitting {} bytes", buf.len());
            let tx = ethernet_device::Tx {
                payload: buf.try_into().unwrap(),
            };
            if let Err(err) = driver_sender.send(tx) {
                warn!("failed to send: {:?}", err);
            }
        }
    };

    let device = NetDevice::new(Box::new(transmit));
    smotcp_log::init();
    let mut server = TcpIp::new(device, mac);

    let mut mainloop = Mainloop::<Context, Message>::new().unwrap();
    mainloop.add_channel(startup_ch, Context::Startup).unwrap();
    mainloop
        .add_channel((driver_sender, driver_receiver), Context::Driver)
        .unwrap();

    loop {
        server.poll(&mut mainloop);
        match mainloop.next() {
            Event::Message {
                ctx: Context::Startup,
                message: Message::NewClient(m),
                ..
            } => {
                let new_ch = m.handle.take::<Channel>().unwrap();
                trace!("got new client: {:?}", new_ch);
                mainloop.add_channel(new_ch, Context::CtrlSocket).unwrap();
            }
            Event::Message {
                ctx: Context::CtrlSocket,
                message: Message::TcpListen(m),
                sender,
            } => {
                match server.tcp_listen(m.port) {
                    Ok(ch) => {
                        if let Err(err) = sender.send(TcpListenReply { listen: ch.into() }) {
                            debug_warn!("failed to send: {:?}", err);
                        }
                    }
                    Err(err) => {
                        warn!("failed to listen: {:?}", err);
                        // TODO:
                    }
                }
            }
            Event::Message {
                ctx: Context::DataSocket(handle),
                message: Message::TcpSend(m),
                ..
            } => {
                server.tcp_send(*handle, m.data.as_slice()).unwrap();
            }
            Event::Message {
                ctx: Context::Driver,
                message: Message::Rx(m),
                ..
            } => {
                trace!(
                    "received {} bytes: {:02x?}",
                    m.payload.len(),
                    &m.payload.as_slice()[0..14]
                );
                server.receive_pkt(m.payload.as_slice());
            }
            ev => {
                warn!("unhandled event: {:?}", ev);
            }
        }
    }
}
