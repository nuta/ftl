use alloc::rc::Rc;
use core::fmt;

use ftl_types::channel::CallId;
use ftl_types::channel::InvokeInline;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::ReadInline;
use ftl_types::channel::WriteInline;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;
use log::warn;

use crate::channel::Channel;
use crate::channel::Reply;
use crate::handle::Handleable;
use crate::interrupt::Interrupt;
use crate::service::Service;
use crate::sink;
use crate::sink::Sink;

enum Object {
    Channel(#[allow(unused)] Rc<Channel>),
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
        if let Err(error) = self.ch.reply(self.call_id, Reply::ReadReply { len }) {
            warn!("failed to complete read: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(send_error) = self.ch.reply(self.call_id, Reply::ErrorReply { error }) {
            warn!("failed to error read: {:?}", send_error);
        }
    }

    pub fn complete_with(&self, data: &[u8]) {
        match self.ch.ool_write(self.call_id, 0, 0, data) {
            Ok(len) => self.complete(len),
            Err(error) => self.error(error),
        }
    }
}

pub struct WriteCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl WriteCompleter {
    pub fn complete(&self, len: usize) {
        if let Err(error) = self.ch.reply(self.call_id, Reply::WriteReply { len }) {
            warn!("failed to complete write: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(send_error) = self.ch.reply(self.call_id, Reply::ErrorReply { error }) {
            warn!("failed to error write: {:?}", send_error);
        }
    }

    pub fn read_data(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }
}

pub struct InvokeCompleter {
    ch: Rc<Channel>,
    kind: u32,
    call_id: CallId,
}

impl InvokeCompleter {
    pub fn complete(self) {
        if let Err(error) = self.ch.reply(self.call_id, Reply::InvokeReply {}) {
            warn!("failed to complete invoke: {:?}", error);
        }
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
        if let Err(send_error) = self.ch.reply(self.call_id, Reply::ErrorReply { error }) {
            warn!("failed to error invoke: {:?}", send_error);
        }
    }

    pub fn complete_with(self, data: &[u8]) {
        if let Err(error) = self.write_bytes(0, data) {
            self.error(error);
            return;
        }

        self.complete();
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
        loop {
            let event = self.sink.wait().unwrap();
            match event {
                sink::Event::CallMessage {
                    ch_id,
                    info,
                    call_id,
                    handles,
                    inline,
                } => {
                    // TODO: Support passing handles through eventloop API.
                    drop(handles);

                    let ch = match self.states.get(&ch_id) {
                        Some(State {
                            object: Object::Channel(ch),
                        }) => ch.clone(),
                        _ => panic!("unknown handle id from sink: {:?}", ch_id),
                    };

                    let request = match info {
                        MessageInfo::READ => {
                            // FIXME: Alignment is not guaranteed.
                            let inline = unsafe { &*(inline.as_ptr() as *const ReadInline) };
                            Request::Read {
                                len: inline.len,
                                completer: ReadCompleter { ch, call_id },
                            }
                        }
                        MessageInfo::WRITE => {
                            // FIXME: Alignment is not guaranteed.
                            let inline = unsafe { &*(inline.as_ptr() as *const WriteInline) };
                            Request::Write {
                                len: inline.len,
                                completer: WriteCompleter { ch, call_id },
                            }
                        }
                        MessageInfo::INVOKE => {
                            // FIXME: Alignment is not guaranteed.
                            let inline = unsafe { &*(inline.as_ptr() as *const InvokeInline) };
                            Request::Invoke {
                                completer: InvokeCompleter {
                                    ch,
                                    kind: inline.kind,
                                    call_id,
                                },
                            }
                        }
                        _ => panic!("unexpected message info: {:?}", info),
                    };

                    return Event::Request(request);
                }
                sink::Event::Irq { handle_id, irq: _ } => {
                    match self.states.get(&handle_id) {
                        Some(State {
                            object: Object::Interrupt(interrupt),
                        }) => return Event::Interrupt { interrupt },
                        _ => panic!("unknown handle id from sink: {:?}", handle_id),
                    }
                }
                sink::Event::Client { ch } => {
                    let ch = Rc::new(ch);
                    self.sink.add(ch.as_ref()).unwrap();
                    self.states.insert(
                        ch.handle().id(),
                        State {
                            object: Object::Channel(ch),
                        },
                    );
                }
                sink::Event::PeerClosed { ch_id } => {
                    todo!("peer closed");
                }
                sink::Event::ReplyMessage { .. } => {
                    panic!("reply messages are not supported in EventLoop")
                }
                sink::Event::Timer { handle_id } => {
                    panic!("unexpected timer event: {:?}", handle_id)
                }
            }
        }
    }
}
