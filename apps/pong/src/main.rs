#![no_std]
#![no_main]

use ftl::application::Event;
use ftl::application::EventLoop;
use ftl::application::RequestEvent;
use ftl::channel::Channel;
use ftl::error::ErrorCode;
use ftl::handle::HandleId;
use ftl::handle::OwnedHandle;
use ftl::log::*;

#[ftl::main]
fn main() {
    let mut eventloop = EventLoop::new().unwrap();

    let ch_id = HandleId::from_raw(1);
    let ch = Channel::from_handle(OwnedHandle::from_raw(ch_id));
    eventloop.add_channel(ch).unwrap();

    loop {
        match eventloop.wait() {
            Event::Request(RequestEvent::Write {
                offset,
                len: _,
                completer,
            }) => {
                let mut buf = [0; 512];
                match completer.read_data(offset, &mut buf) {
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
            Event::Request(RequestEvent::Open { completer }) => {
                completer.error(ErrorCode::Unsupported);
            }
            Event::Request(RequestEvent::Read {
                offset: _,
                len: _,
                completer,
            }) => {
                completer.error(ErrorCode::Unsupported);
            }
            ev => {
                warn!("[pong] unhandled event: {:?}", ev);
            }
        }
    }
}
