#![allow(unused)]

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::marker::PhantomData;

use ftl_types::channel::CallId;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::MessageInlineBody;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::handle::Handleable;
use crate::interrupt::Interrupt;
use crate::sink;
use crate::sink::Sink;
use crate::thread::Thread;
use crate::time::Timer;

#[derive(Debug)]
pub struct OpenCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

#[derive(Debug)]
pub struct ReadCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

#[derive(Debug)]
pub struct WriteCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

#[derive(Debug)]
pub struct ReadUriCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

#[derive(Debug)]
pub struct WriteUriCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

#[derive(Debug)]
pub enum Event<'a, C, K> {
    Open {
        ctx: &'a mut C,
        completer: OpenCompleter,
    },
    Read {
        ctx: &'a mut C,
        completer: ReadCompleter,
        offset: usize,
        len: usize,
    },
    Write {
        ctx: &'a mut C,
        completer: WriteCompleter,
        offset: usize,
        len: usize,
    },
    ReadUri {
        ctx: &'a mut C,
        completer: ReadUriCompleter,
    },
    WriteUri {
        ctx: &'a mut C,
        completer: WriteUriCompleter,
    },
    OpenReply {
        ch: &'a Rc<Channel>,
        cookie: K,
    },
    ReadReply {
        ch: &'a Rc<Channel>,
        cookie: K,
        len: usize,
    },
    WriteReply {
        ch: &'a Rc<Channel>,
        cookie: K,
        len: usize,
    },
    ReadUriReply {
        ch: &'a Rc<Channel>,
        cookie: K,
        len: usize,
    },
    WriteUriReply {
        ch: &'a Rc<Channel>,
        cookie: K,
        len: usize,
    },
    ErrorReply {
        ch: &'a Rc<Channel>,
        cookie: K,
        error: ErrorCode,
    },
    UnknownMessage {
        ctx: &'a mut C,
        info: MessageInfo,
    },
    SinkError(ErrorCode),
}

#[derive(Debug)]
pub enum Error {
    SinkCreate(ErrorCode),
    SinkRemove(ErrorCode),
    SinkAdd(ErrorCode),
}

enum Object {
    Channel(Rc<Channel>),
    Interrupt(Rc<Interrupt>),
    Timer(Rc<Timer>),
    Thread(Rc<Thread>),
}

struct Entry<C> {
    object: Object,
    ctx: C,
}

pub trait SmartPointer {
    fn into_raw(self) -> usize;
    fn from_raw(raw: usize) -> Self;
}

impl<T> SmartPointer for Box<T> {
    fn into_raw(self) -> usize {
        Box::into_raw(self) as usize
    }

    fn from_raw(raw: usize) -> Self {
        unsafe { Box::from_raw(raw as *mut T) }
    }
}

impl<T> SmartPointer for Rc<T> {
    fn into_raw(self) -> usize {
        Rc::into_raw(self) as usize
    }

    fn from_raw(raw: usize) -> Self {
        unsafe { Rc::from_raw(raw as *mut T) }
    }
}

pub struct EventLoop<C, K: SmartPointer> {
    sink: Sink,
    entries: HashMap<HandleId, Entry<C>>,
    _pd: PhantomData<K>,
}

impl<C, K: SmartPointer> EventLoop<C, K> {
    pub fn new() -> Result<Self, Error> {
        let sink = Sink::new().map_err(Error::SinkCreate)?;
        Ok(Self {
            sink,
            entries: HashMap::new(),
            _pd: PhantomData,
        })
    }

    pub fn add_channel(&mut self, channel: impl Into<Rc<Channel>>, ctx: C) -> Result<(), Error> {
        let ch = channel.into();
        self.sink.add(ch.as_ref()).map_err(Error::SinkAdd)?;
        self.entries.insert(
            ch.handle().id(),
            Entry {
                object: Object::Channel(ch),
                ctx,
            },
        );
        Ok(())
    }

    pub fn add_interrupt(
        &mut self,
        interrupt: impl Into<Rc<Interrupt>>,
        ctx: C,
    ) -> Result<(), Error> {
        let interrupt = interrupt.into();
        self.sink.add(interrupt.as_ref()).map_err(Error::SinkAdd)?;
        self.entries.insert(
            interrupt.handle().id(),
            Entry {
                object: Object::Interrupt(interrupt),
                ctx,
            },
        );
        Ok(())
    }

    pub fn add_timer(&mut self, timer: impl Into<Rc<Timer>>, ctx: C) -> Result<(), Error> {
        let timer = timer.into();
        self.sink.add(timer.as_ref()).map_err(Error::SinkAdd)?;
        self.entries.insert(
            timer.handle().id(),
            Entry {
                object: Object::Timer(timer),
                ctx,
            },
        );
        Ok(())
    }

    pub fn add_thread(&mut self, thread: impl Into<Rc<Thread>>, ctx: C) -> Result<(), Error> {
        let thread = thread.into();
        self.sink.add(thread.as_ref()).map_err(Error::SinkAdd)?;
        self.entries.insert(
            thread.handle().id(),
            Entry {
                object: Object::Thread(thread),
                ctx,
            },
        );
        Ok(())
    }

    pub fn remove(&mut self, id: HandleId) -> Result<(), Error> {
        self.sink.remove(id).map_err(Error::SinkRemove)?;
        self.entries.remove(&id);
        Ok(())
    }

    pub fn wait(&mut self) -> Event<'_, C, K> {
        let result = self.sink.wait();
        match result {
            Ok(sink::Event::CallMessage {
                ch_id,
                info,
                call_id,
                handles,
                inline,
            }) => {
                let (ch, ctx) = match self.entries.get_mut(&ch_id) {
                    Some(Entry {
                        object: Object::Channel(ch),
                        ctx,
                    }) => (ch.clone(), ctx),
                    _ => panic!("unknown handle id from sink: {:?}", ch_id),
                };

                // FIXME: Guarantee the alignment of the inline body.
                let inline_body = unsafe { &*(inline.as_ptr() as *const MessageInlineBody) };
                match info {
                    MessageInfo::OPEN => {
                        Event::Open {
                            ctx,
                            completer: OpenCompleter { ch, call_id },
                        }
                    }
                    MessageInfo::READ => {
                        Event::Read {
                            ctx,
                            offset: unsafe { inline_body.read.offset },
                            len: unsafe { inline_body.read.len },
                            completer: ReadCompleter { ch, call_id },
                        }
                    }
                    MessageInfo::WRITE => {
                        Event::Write {
                            ctx,
                            offset: unsafe { inline_body.write.offset },
                            len: unsafe { inline_body.write.len },
                            completer: WriteCompleter { ch, call_id },
                        }
                    }
                    MessageInfo::READ_URI => {
                        Event::ReadUri {
                            ctx,
                            completer: ReadUriCompleter { ch, call_id },
                        }
                    }
                    MessageInfo::WRITE_URI => {
                        Event::WriteUri {
                            ctx,
                            completer: WriteUriCompleter { ch, call_id },
                        }
                    }
                    _ => Event::UnknownMessage { ctx, info },
                }
            }
            Ok(sink::Event::ReplyMessage {
                ch_id,
                info,
                cookie,
                handles,
                inline,
            }) => {
                let (ch, ctx) = match self.entries.get_mut(&ch_id) {
                    Some(Entry {
                        object: Object::Channel(ch),
                        ctx,
                    }) => (ch, ctx),
                    _ => panic!("unknown handle id from sink: {:?}", ch_id),
                };

                // FIXME: Guarantee the alignment of the inline body.
                let inline_body = unsafe { &*(inline.as_ptr() as *const MessageInlineBody) };
                let cookie = K::from_raw(cookie);
                match info {
                    MessageInfo::OPEN_REPLY => Event::OpenReply { ch, cookie },
                    MessageInfo::READ_REPLY => {
                        let len = unsafe { inline_body.read_reply.len };
                        Event::ReadReply { ch, cookie, len }
                    }
                    MessageInfo::WRITE_REPLY => {
                        let len = unsafe { inline_body.write_reply.len };
                        Event::WriteReply { ch, cookie, len }
                    }
                    MessageInfo::READ_URI_REPLY => {
                        let len = unsafe { inline_body.read_uri_reply.len };
                        Event::ReadUriReply { ch, cookie, len }
                    }
                    MessageInfo::WRITE_URI_REPLY => {
                        let len = unsafe { inline_body.write_uri_reply.len };
                        Event::WriteUriReply { ch, cookie, len }
                    }
                    MessageInfo::ERROR_REPLY => {
                        let error = unsafe { inline_body.error_reply.error };
                        Event::ErrorReply { ch, cookie, error }
                    }
                    _ => Event::UnknownMessage { ctx, info },
                }
            }
            Ok(sink::Event::PeerClosed { ch_id }) => {
                todo!()
            }
            Ok(sink::Event::Irq { handle_id, irq }) => {
                todo!()
            }
            Ok(sink::Event::Timer { handle_id }) => {
                todo!()
            }
            Ok(sink::Event::SandboxedSyscall { thread_id, raw }) => {
                todo!()
            }
            Err(error) => Event::SinkError(error),
        }
    }
}
