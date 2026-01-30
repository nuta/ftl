use alloc::rc::Rc;
use alloc::vec::Vec;

use ftl_types::channel::MSGTYPE_ERROR_REPLY;
use ftl_types::channel::MSGTYPE_READ;
use ftl_types::channel::MSGTYPE_READ_REPLY;
use ftl_types::channel::MSGTYPE_WRITE;
use ftl_types::channel::MSGTYPE_WRITE_REPLY;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::sink::Event;
use crate::sink::Sink;

pub struct Completer {
    ch: Rc<Channel>,
}

pub trait Application {
    fn init() -> Self;
    fn open(&mut self, ch: &Rc<Channel>, uri: &[u8], completer: Completer);
    fn read(&mut self, ch: &Rc<Channel>, len: usize, completer: Completer);
    fn write(&mut self, ch: &Rc<Channel>, buf: Vec<u8>, completer: Completer);
    fn open_reply(&mut self, ch: &Rc<Channel>, new_ch: Channel);
    fn read_reply(&mut self, ch: &Rc<Channel>, buf: Vec<u8>);
    fn write_reply(&mut self, ch: &Rc<Channel>, len: usize);
    fn error_reply(&mut self, ch: &Rc<Channel>, error: ErrorCode);
}

pub fn main<A: Application>(app: A) {
    let mut app = A::init();
    let mut sink = Sink::new().unwrap();
    let mut channels: HashMap<HandleId, Rc<Channel>> = HashMap::new();
    loop {
        match sink.pop().expect("failed to read an event from sink") {
            Event::Message { id, msginfo } => {
                let ch = channels.get_mut(&id).unwrap();
                let mut buf = Vec::with_capacity(msginfo.len());
                let mut handles = [HandleId::new(0); 1];

                match ch.recv(&mut buf, &mut handles) {
                    Ok(msginfo) => {
                        // TODO: set len
                        debug_assert_eq!(msginfo.len(), buf.len());

                        match msginfo.ty() {
                            MSGTYPE_ERROR_REPLY => {
                                let error = todo!();
                                app.error_reply(ch, error);
                            }
                            MSGTYPE_READ => {
                                let completer = Completer { ch: Rc::clone(ch) };
                                let len = todo!();
                                app.read(ch, len, completer);
                            }
                            MSGTYPE_READ_REPLY => {
                                app.read_reply(ch, buf);
                            }
                            MSGTYPE_WRITE => {
                                let completer = Completer { ch: Rc::clone(ch) };
                                app.write(ch, buf, completer);
                            }
                            MSGTYPE_WRITE_REPLY => {
                                let written_len = todo!();
                                app.write_reply(ch, written_len);
                            }
                            _ => {
                                println!("unknown message type: {:?}", msginfo.ty());
                            }
                        }
                    }
                    Err(err) => {
                        //
                    }
                }
            }
        }
    }
}
