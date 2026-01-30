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
    fn read(&mut self, ch: &Rc<Channel>, off: usize, len: usize, completer: Completer);
    fn write(&mut self, ch: &Rc<Channel>, off: usize, buf: Vec<u8>, completer: Completer);
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
                }
            }
        }
    }
}
