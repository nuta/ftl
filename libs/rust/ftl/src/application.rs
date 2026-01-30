use alloc::rc::Rc;

use ftl_types::channel::CallId;

use crate::buffer::Buffer;
use crate::channel::Channel;

pub struct Completer {
    ch: Rc<Channel>,
    call_id: CallId,
}

pub struct ReadCompleter {}
pub struct WriteCompleter {}

pub trait Application {
    fn read(&mut self, ch: &Rc<Channel>, buf: Buffer, len: usize, completer: ReadCompleter);
    fn write(&mut self, ch: &Rc<Channel>, buf: Buffer, len: usize, completer: WriteCompleter);
}
