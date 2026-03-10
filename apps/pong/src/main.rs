#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::error::ErrorCode;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::handle::HandleId;
use ftl::handle::OwnedHandle;
use ftl::log::*;

#[ftl::main]
fn main() {
    let mut eventloop = EventLoop::new().unwrap();

    let ch_id = HandleId::from_raw(1);
    let ch = Channel::from_handle(OwnedHandle::from_raw(ch_id));
    eventloop.add_channel(ch, ()).unwrap();

    loop {
        match eventloop.wait() {
            Event::Write {
                offset, completer, ..
            } => {
                let mut buf = [0; 512];
                match completer.read(offset, &mut buf) {
                    Ok(len) => {
                        trace!(
                            "[pong] OOL read ({len} bytes): {:?}",
                            core::str::from_utf8(&buf[..len])
                        );
                        completer.complete(len);
                    }
                    Err(error) => {
                        warn!("[pong] failed to read write payload: {:?}", error);
                        completer.error(error);
                    }
                }
            }
            Event::Open { completer, .. } => {
                completer.error(ErrorCode::Unsupported);
            }
            Event::Read { completer, .. } => {
                completer.error(ErrorCode::Unsupported);
            }
            ev => {
                warn!("[pong] unhandled event: {:?}", ev);
            }
        }
    }
}
