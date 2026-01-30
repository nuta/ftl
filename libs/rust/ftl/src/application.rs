use alloc::rc::Rc;
use alloc::vec::Vec;

use hashbrown::HashMap;

use crate::channel::Channel;
use crate::sink::Event;
use crate::sink::Sink;

pub struct Completer {
    ch: Rc<Channel>,
}

pub enum Object<C> {
    Channel { ch: Rc<Channel>, ctx: C },
}

pub trait Application {
    fn init() -> Self;
    fn open(&mut self, ch: &Rc<Channel>, uri: &[u8], completer: Completer);
    fn read(&mut self, ch: &Rc<Channel>, len: usize, completer: Completer);
    fn write(&mut self, ch: &Rc<Channel>, buf: &[u8], completer: Completer);
    fn open_reply(&mut self, ch: &Rc<Channel>, new_ch: Channel);
    fn read_reply(&mut self, ch: &Rc<Channel>, buf: Vec<u8>);
    fn write_reply(&mut self, ch: &Rc<Channel>, len: usize);
}

pub fn main<A: Application>(app: A) {
    let mut app = A::init();
    let mut sink = Sink::new().unwrap();
    let mut objects = HashMap::new();
    loop {
        match sink.pop().expect("failed to read an event from sink") {
            Event::Message { id, msginfo } => {
                let buf = Vec::with_capacity(msginfo.len());
                let mut handles = [0; 1];
                match ch.recv(buf, handles) {
                    Ok(msginfo) => {
                        //
                    }
                }
            }
        }
    }
}
