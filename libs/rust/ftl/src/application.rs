use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::buffer::Buffer;
use crate::channel::Channel;

pub struct Completer {
    ch: Rc<Channel>,
}

pub trait Application {
    fn open(&mut self, uri: &[u8], completer: Completer);
    fn read(&mut self, ch: &Rc<Channel>, len: usize, completer: Completer);
    fn write(&mut self, ch: &Rc<Channel>, buf: &[u8], completer: Completer);
    fn open_reply(&mut self, ch: &Rc<Channel>, new_ch: Channel);
    fn read_reply(&mut self, ch: &Rc<Channel>, buf: Vec<u8>);
    fn write_reply(&mut self, ch: &Rc<Channel>, len: usize);
}
