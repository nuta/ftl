#![no_std]
#![no_main]

use ftl::channel::Channel;
use ftl::eventloop::Event;
use ftl::eventloop::EventLoop;
use ftl::eventloop::Request;
use ftl::log::*;

#[derive(Debug)]
enum Context {
    Listener,
    Client,
}

#[ftl::main]
fn main() {
    let mut eventloop: EventLoop<Context, ()> = EventLoop::new().unwrap();
    let ch = Channel::register("pong").unwrap();
    eventloop.add_channel(ch, Context::Listener).unwrap();

    loop {
        match eventloop.wait() {
            Event::Request {
                ctx: Context::Listener,
                request: Request::Open(request),
            } => {
                let (our_ch, their_ch) = match Channel::new() {
                    Ok(pair) => pair,
                    Err(error) => {
                        request.reply_error(error);
                        continue;
                    }
                };

                if let Err(error) = eventloop.add_channel(our_ch, Context::Client) {
                    request.reply_error(error.into());
                    continue;
                }

                request.reply(their_ch);
            }
            Event::Request {
                ctx: Context::Client,
                request: Request::Write(request),
                ..
            } => {
                let mut buf = [0; 512];
                match request.read_at(&mut buf, request.offset()) {
                    Ok(len) => {
                        trace!(
                            "body read ({len} bytes): {:?}",
                            core::str::from_utf8(&buf[..len])
                        );
                        request.reply(len);
                    }
                    Err(error) => {
                        warn!("failed to read write payload: {:?}", error);
                        request.reply_error(error);
                    }
                }
            }
            ev => {
                warn!("unhandled event: {:?}", ev);
            }
        }
    }
}
