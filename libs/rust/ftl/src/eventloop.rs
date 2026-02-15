use alloc::rc::Rc;
use core::fmt;

use ftl_types::channel::CallId;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::handle::Handleable;
use crate::interrupt::Interrupt;
use crate::service::Service;
use crate::sink;
use crate::sink::Sink;

enum Object {
    Interrupt(#[allow(unused)] Rc<Interrupt>),
    Service(#[allow(unused)] Rc<Service>),
}

struct State {
    object: Object,
}

pub struct ReadCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl ReadCompleter {
    pub fn complete(&self, len: usize) {
        todo!();
    }

    pub fn error(&self, error: ErrorCode) {
        todo!();
    }

    pub fn complete_with(&self, data: &[u8]) {
        todo!();
    }
}

pub struct WriteCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl WriteCompleter {
    pub fn complete(&self, len: usize) {
        todo!();
    }

    pub fn error(&self, error: ErrorCode) {
        todo!();
    }

    pub fn read_data(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }

    pub fn complete_with(&self, data: &[u8]) {
        todo!();
    }
}

pub struct InvokeCompleter {
    ch: Rc<Channel>,
    kind: u32,
    call_id: CallId,
}

impl InvokeCompleter {
    pub fn complete(&self, len: usize) {
        todo!();
    }

    pub fn kind(&self) -> u32 {
        self.kind
    }

    pub fn read_bytes(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }

    pub fn write_bytes(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 1, offset, data)
    }

    pub fn error(&self, error: ErrorCode) {
        todo!();
    }

    pub fn complete_with(&self, data: &[u8]) {
        todo!();
    }
}

pub enum Request {
    Read {
        len: usize,
        completer: ReadCompleter,
    },
    Write {
        len: usize,
        completer: WriteCompleter,
    },
    Invoke {
        completer: InvokeCompleter,
    },
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Request::Read { len, .. } => f.debug_tuple("Read").field(len).finish(),
            Request::Write { len, .. } => f.debug_tuple("Write").field(len).finish(),
            Request::Invoke { .. } => f.debug_tuple("Invoke").finish(),
        }
    }
}

#[derive(Debug)]
pub enum Event<'a> {
    Request(Request),
    Interrupt { interrupt: &'a Rc<Interrupt> },
    Connect(Channel),
}

pub struct EventLoop {
    sink: Sink,
    states: HashMap<HandleId, State>,
}

impl EventLoop {
    pub fn new() -> Result<Self, ErrorCode> {
        let sink = Sink::new()?;
        Ok(Self {
            sink,
            states: HashMap::new(),
        })
    }

    pub fn add_interrupt<T: Into<Rc<Interrupt>>>(&mut self, interrupt: T) -> Result<(), ErrorCode> {
        let object = interrupt.into();
        self.sink.add(object.as_ref())?;
        self.states.insert(
            object.handle().id(),
            State {
                object: Object::Interrupt(object),
            },
        );
        Ok(())
    }

    pub fn add_service<T: Into<Rc<Service>>>(&mut self, service: T) -> Result<(), ErrorCode> {
        let object = service.into();
        self.sink.add(object.as_ref())?;
        self.states.insert(
            object.handle().id(),
            State {
                object: Object::Service(object),
            },
        );
        Ok(())
    }

    pub fn wait(&mut self) -> Event<'_> {
        let event = self.sink.wait();
        match event {
            Ok(sink::Event::CallMessage {
                ch_id,
                info,
                call_id,
                handles,
                inline,
                //
            }) => {
                todo!();
            }
            Ok(sink::Event::Irq { handle_id, irq }) => {
                todo!();
            }
            Ok((sink::Event::Client { ch })) => Event::Connect(ch),
            _ => {
                todo!();
            }
        }
    }
}
