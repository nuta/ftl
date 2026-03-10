use alloc::rc::Rc;
use core::fmt;
use core::ptr;

use ftl_types::channel::CallId;
use ftl_types::channel::ErrorReplyInline;
use ftl_types::channel::INLINE_LEN_MAX;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::OpenInline;
use ftl_types::channel::OpenReplyInline;
use ftl_types::channel::ReadInline;
use ftl_types::channel::ReadReplyInline;
use ftl_types::channel::ReadUriInline;
use ftl_types::channel::ReadUriReplyInline;
use ftl_types::channel::WriteInline;
use ftl_types::channel::WriteReplyInline;
use ftl_types::channel::WriteUriInline;
use ftl_types::channel::WriteUriReplyInline;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;
use log::warn;

use crate::channel::Buffer;
use crate::channel::BufferMut;
use crate::channel::Channel;
use crate::channel::Cookie;
use crate::channel::Reply as ChannelReply;
use crate::handle::Handleable;
use crate::interrupt::Interrupt;
use crate::sink;
use crate::sink::SandboxedSyscallEvent;
use crate::sink::Sink;
use crate::thread::Thread;
use crate::time::Timer;

enum Object {
    Channel(#[allow(unused)] Rc<Channel>),
    Interrupt(#[allow(unused)] Rc<Interrupt>),
    Timer(#[allow(unused)] Rc<Timer>),
    Thread(#[allow(unused)] Rc<Thread>),
}

struct State {
    object: Object,
}

pub struct ReadCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl ReadCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn complete(&self, len: usize) {
        if let Err(error) = self.ch.reply(self.call_id, ChannelReply::ReadReply { len }) {
            warn!("failed to complete read: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(send_error) = self
            .ch
            .reply(self.call_id, ChannelReply::ErrorReply { error })
        {
            warn!("failed to error read: {:?}", send_error);
        }
    }

    pub fn write_data(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 0, offset, data)
    }

    pub fn complete_with(&self, data: &[u8]) {
        match self.write_data(0, data) {
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
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn complete(&self, len: usize) {
        if let Err(error) = self
            .ch
            .reply(self.call_id, ChannelReply::WriteReply { len })
        {
            warn!("failed to complete write: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(send_error) = self
            .ch
            .reply(self.call_id, ChannelReply::ErrorReply { error })
        {
            warn!("failed to error write: {:?}", send_error);
        }
    }

    pub fn read_data(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }
}

pub struct ReadUriCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl ReadUriCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn complete(&self, len: usize) {
        if let Err(error) = self
            .ch
            .reply(self.call_id, ChannelReply::ReadUriReply { len })
        {
            warn!("failed to complete read uri: {:?}", error);
        }
    }

    pub fn read_uri(&self, offset: usize, uri: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, uri)
    }

    pub fn write_data(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 1, offset, data)
    }

    pub fn complete_with(&self, data: &[u8]) {
        match self.write_data(0, data) {
            Ok(len) => self.complete(len),
            Err(error) => self.error(error),
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(send_error) = self
            .ch
            .reply(self.call_id, ChannelReply::ErrorReply { error })
        {
            warn!("failed to error read uri: {:?}", send_error);
        }
    }
}

pub struct WriteUriCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl WriteUriCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn complete(&self, len: usize) {
        if let Err(error) = self
            .ch
            .reply(self.call_id, ChannelReply::WriteUriReply { len })
        {
            warn!("failed to complete write uri: {:?}", error);
        }
    }

    pub fn read_uri(&self, offset: usize, uri: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, uri)
    }

    pub fn read_data(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 1, offset, data)
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(send_error) = self
            .ch
            .reply(self.call_id, ChannelReply::ErrorReply { error })
        {
            warn!("failed to error write uri: {:?}", send_error);
        }
    }
}

pub struct OpenCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl OpenCompleter {
    pub fn channel(&self) -> &Rc<Channel> {
        &self.ch
    }

    pub fn handle_id(&self) -> HandleId {
        self.ch.handle().id()
    }

    pub fn read_uri(&self, offset: usize, uri: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, uri)
    }

    pub fn complete(&self, ch: Channel) {
        if let Err(error) = self.ch.reply(self.call_id, ChannelReply::OpenReply { ch }) {
            warn!("failed to complete open: {:?}", error);
        }
    }

    pub fn error(&self, error: ErrorCode) {
        if let Err(send_error) = self
            .ch
            .reply(self.call_id, ChannelReply::ErrorReply { error })
        {
            warn!("failed to error open: {:?}", send_error);
        }
    }
}

pub enum RequestEvent {
    Open {
        completer: OpenCompleter,
    },
    Read {
        offset: usize,
        len: usize,
        completer: ReadCompleter,
    },
    Write {
        offset: usize,
        len: usize,
        completer: WriteCompleter,
    },
    ReadUri {
        offset: usize,
        len: usize,
        completer: ReadUriCompleter,
    },
    WriteUri {
        offset: usize,
        len: usize,
        completer: WriteUriCompleter,
    },
}

impl fmt::Debug for RequestEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestEvent::Open { .. } => f.debug_tuple("Open").finish(),
            RequestEvent::Read { offset, len, .. } => {
                f.debug_struct("Read")
                    .field("offset", offset)
                    .field("len", len)
                    .finish()
            }
            RequestEvent::Write { offset, len, .. } => {
                f.debug_struct("Write")
                    .field("offset", offset)
                    .field("len", len)
                    .finish()
            }
            RequestEvent::ReadUri { offset, len, .. } => {
                f.debug_struct("ReadUri")
                    .field("offset", offset)
                    .field("len", len)
                    .finish()
            }
            RequestEvent::WriteUri { offset, len, .. } => {
                f.debug_struct("WriteUri")
                    .field("offset", offset)
                    .field("len", len)
                    .finish()
            }
        }
    }
}

pub enum ReplyEvent {
    Open {
        ch: Rc<Channel>,
        uri: Buffer,
        new_ch: Channel,
    },
    Read {
        ch: Rc<Channel>,
        buf: BufferMut,
        len: usize,
    },
    Write {
        ch: Rc<Channel>,
        buf: Buffer,
        len: usize,
    },
    ReadUri {
        ch: Rc<Channel>,
        uri: Buffer,
        buf: BufferMut,
        len: usize,
    },
    WriteUri {
        ch: Rc<Channel>,
        uri: Buffer,
        buf: Buffer,
        len: usize,
    },
    Error {
        ch: Rc<Channel>,
        error: ErrorCode,
    },
}

impl ReplyEvent {
    pub fn channel(&self) -> &Rc<Channel> {
        match self {
            ReplyEvent::Open { ch, .. }
            | ReplyEvent::Read { ch, .. }
            | ReplyEvent::Write { ch, .. }
            | ReplyEvent::ReadUri { ch, .. }
            | ReplyEvent::WriteUri { ch, .. }
            | ReplyEvent::Error { ch, .. } => ch,
        }
    }

    pub fn handle_id(&self) -> HandleId {
        self.channel().handle().id()
    }
}

impl fmt::Debug for ReplyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplyEvent::Open { .. } => f.debug_tuple("Open").finish(),
            ReplyEvent::Read { len, .. } => f.debug_tuple("Read").field(len).finish(),
            ReplyEvent::Write { len, .. } => f.debug_tuple("Write").field(len).finish(),
            ReplyEvent::ReadUri { len, .. } => f.debug_tuple("ReadUri").field(len).finish(),
            ReplyEvent::WriteUri { len, .. } => f.debug_tuple("WriteUri").field(len).finish(),
            ReplyEvent::Error { error, .. } => f.debug_tuple("Error").field(error).finish(),
        }
    }
}

pub enum Event {
    Request(RequestEvent),
    Reply(ReplyEvent),
    Interrupt {
        interrupt: Rc<Interrupt>,
    },
    Timer {
        timer: Rc<Timer>,
    },
    PeerClosed {
        ch: Rc<Channel>,
    },
    SandboxedSyscall {
        thread: Rc<Thread>,
        regs: SandboxedSyscallEvent,
    },
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Request(request) => f.debug_tuple("Request").field(request).finish(),
            Event::Reply(reply) => f.debug_tuple("Reply").field(reply).finish(),
            Event::Interrupt { interrupt } => f.debug_tuple("Interrupt").field(interrupt).finish(),
            Event::Timer { .. } => f.debug_tuple("Timer").finish(),
            Event::PeerClosed { ch } => f.debug_tuple("PeerClosed").field(ch).finish(),
            Event::SandboxedSyscall { thread, regs } => {
                f.debug_tuple("Syscall").field(thread).field(regs).finish()
            }
        }
    }
}

fn read_inline<T>(inline: &[u8; INLINE_LEN_MAX]) -> T {
    // SAFETY: Inline data is provided by the kernel and sized by MessageInfo.
    unsafe { ptr::read_unaligned(inline.as_ptr() as *const T) }
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

    pub fn sink(&self) -> &Sink {
        &self.sink
    }

    pub fn add_channel<T: Into<Rc<Channel>>>(&mut self, ch: T) -> Result<(), ErrorCode> {
        let object = ch.into();
        self.sink.add(object.as_ref())?;
        self.states.insert(
            object.handle().id(),
            State {
                object: Object::Channel(object),
            },
        );
        Ok(())
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

    pub fn add_timer<T: Into<Rc<Timer>>>(&mut self, timer: T) -> Result<(), ErrorCode> {
        let object = timer.into();
        self.sink.add(object.as_ref())?;
        self.states.insert(
            object.handle().id(),
            State {
                object: Object::Timer(object),
            },
        );
        Ok(())
    }

    pub fn add_thread<T: Into<Rc<Thread>>>(&mut self, thread: T) -> Result<(), ErrorCode> {
        let object = thread.into();
        self.sink.add(object.as_ref())?;
        self.states.insert(
            object.handle().id(),
            State {
                object: Object::Thread(object),
            },
        );
        Ok(())
    }

    pub fn remove(&mut self, id: HandleId) -> Result<(), ErrorCode> {
        self.sink.remove(id)?;
        self.states.remove(&id);
        Ok(())
    }

    pub fn wait(&mut self) -> Event {
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
                    debug_assert!(handles.is_empty());
                    let ch = match self.states.get(&ch_id) {
                        Some(State {
                            object: Object::Channel(ch),
                        }) => ch.clone(),
                        _ => panic!("unknown handle id from sink: {:?}", ch_id),
                    };

                    let request = match info {
                        MessageInfo::OPEN => {
                            let _inline: OpenInline = read_inline(&inline);
                            RequestEvent::Open {
                                completer: OpenCompleter { ch, call_id },
                            }
                        }
                        MessageInfo::READ => {
                            let inline: ReadInline = read_inline(&inline);
                            RequestEvent::Read {
                                offset: inline.offset,
                                len: inline.len,
                                completer: ReadCompleter { ch, call_id },
                            }
                        }
                        MessageInfo::WRITE => {
                            let inline: WriteInline = read_inline(&inline);
                            RequestEvent::Write {
                                offset: inline.offset,
                                len: inline.len,
                                completer: WriteCompleter { ch, call_id },
                            }
                        }
                        MessageInfo::READ_URI => {
                            let inline: ReadUriInline = read_inline(&inline);
                            RequestEvent::ReadUri {
                                offset: inline.offset,
                                len: inline.len,
                                completer: ReadUriCompleter { ch, call_id },
                            }
                        }
                        MessageInfo::WRITE_URI => {
                            let inline: WriteUriInline = read_inline(&inline);
                            RequestEvent::WriteUri {
                                offset: inline.offset,
                                len: inline.len,
                                completer: WriteUriCompleter { ch, call_id },
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
                        }) => {
                            return Event::Interrupt {
                                interrupt: interrupt.clone(),
                            };
                        }
                        _ => panic!("unknown handle id from sink: {:?}", handle_id),
                    }
                }
                sink::Event::PeerClosed { ch_id } => {
                    match self.states.get(&ch_id) {
                        Some(State {
                            object: Object::Channel(ch),
                        }) => {
                            return Event::PeerClosed { ch: ch.clone() };
                        }
                        _ => panic!("unknown handle id from sink: {:?}", ch_id),
                    }
                }
                sink::Event::ReplyMessage {
                    ch_id,
                    info,
                    cookie,
                    mut handles,
                    inline,
                } => {
                    let ch = match self.states.get(&ch_id) {
                        Some(State {
                            object: Object::Channel(ch),
                        }) => ch.clone(),
                        _ => panic!("unknown handle id from sink: {:?}", ch_id),
                    };

                    // FIXME: Cookie is not guaranteed to be Box<Cookie>.
                    let cookie = unsafe { Cookie::from_raw(cookie) };
                    let reply = match info {
                        MessageInfo::OPEN_REPLY => {
                            let _inline: OpenReplyInline = read_inline(&inline);
                            let Cookie::Buffer(uri) = *cookie else {
                                panic!("unexpected cookie type");
                            };
                            let new_ch = Channel::from_handle(handles.pop().unwrap());
                            ReplyEvent::Open { ch, uri, new_ch }
                        }
                        MessageInfo::READ_REPLY => {
                            let inline: ReadReplyInline = read_inline(&inline);
                            let Cookie::BufferMut(buf) = *cookie else {
                                panic!("unexpected cookie type");
                            };
                            ReplyEvent::Read {
                                ch,
                                buf,
                                len: inline.len,
                            }
                        }
                        MessageInfo::WRITE_REPLY => {
                            let inline: WriteReplyInline = read_inline(&inline);
                            let Cookie::Buffer(buf) = *cookie else {
                                panic!("unexpected cookie type");
                            };
                            ReplyEvent::Write {
                                ch,
                                buf,
                                len: inline.len,
                            }
                        }
                        MessageInfo::READ_URI_REPLY => {
                            let inline: ReadUriReplyInline = read_inline(&inline);
                            let Cookie::ReadUri(uri, buf) = *cookie else {
                                panic!("unexpected cookie type");
                            };
                            ReplyEvent::ReadUri {
                                ch,
                                uri,
                                buf,
                                len: inline.len,
                            }
                        }
                        MessageInfo::WRITE_URI_REPLY => {
                            let inline: WriteUriReplyInline = read_inline(&inline);
                            let Cookie::WriteUri(uri, buf) = *cookie else {
                                panic!("unexpected cookie type");
                            };
                            ReplyEvent::WriteUri {
                                ch,
                                uri,
                                buf,
                                len: inline.len,
                            }
                        }
                        MessageInfo::ERROR_REPLY => {
                            let inline: ErrorReplyInline = read_inline(&inline);
                            ReplyEvent::Error {
                                ch,
                                error: inline.error,
                            }
                        }
                        _ => panic!("unexpected message info: {:?}", info),
                    };

                    return Event::Reply(reply);
                }
                sink::Event::Timer { handle_id } => {
                    match self.states.get(&handle_id) {
                        Some(State {
                            object: Object::Timer(timer),
                        }) => {
                            return Event::Timer {
                                timer: timer.clone(),
                            };
                        }
                        _ => panic!("unknown handle id from sink: {:?}", handle_id),
                    }
                }
                sink::Event::SandboxedSyscall { thread_id, raw } => {
                    match self.states.get(&thread_id) {
                        Some(State {
                            object: Object::Thread(thread),
                        }) => {
                            return Event::SandboxedSyscall {
                                thread: thread.clone(),
                                regs: raw,
                            };
                        }
                        _ => panic!("unknown handle id from sink: {:?}", thread_id),
                    }
                }
            }
        }
    }
}
