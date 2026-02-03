use alloc::rc::Rc;

use ftl_types::channel::CallId;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::OpenInline;
use ftl_types::channel::OpenReplyInline;
use ftl_types::channel::ReadInline;
use ftl_types::channel::ReadReplyInline;
use ftl_types::channel::WriteInline;
use ftl_types::channel::WriteReplyInline;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Buffer;
use crate::channel::BufferMut;
use crate::channel::Channel;
use crate::channel::Cookie;
use crate::channel::Reply;
use crate::handle::Handleable;
use crate::interrupt::Interrupt;
use crate::sink::Event;
use crate::sink::Sink;

pub struct OpenCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl OpenCompleter {
    fn new(channel: Rc<Channel>, call_id: CallId) -> Self {
        Self {
            ch: channel,
            call_id,
        }
    }

    pub fn read_uri(&self, offset: usize, uri: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, uri)
    }

    pub fn error(self, error: ErrorCode) {
        let reply = Reply::ErrorReply { error };
        if let Err(err) = self.ch.reply(self.call_id, reply) {
            println!("failed to complete open: {err:?}");
        }
    }

    pub fn complete(self, ch: Channel) {
        let reply = Reply::OpenReply { ch };
        if let Err(err) = self.ch.reply(self.call_id, reply) {
            println!("failed to complete open: {err:?}");
        }
    }
}

pub struct ReadCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl ReadCompleter {
    fn new(channel: Rc<Channel>, call_id: CallId) -> Self {
        Self {
            ch: channel,
            call_id,
        }
    }

    pub fn write_data(&self, offset: usize, data: &[u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_write(self.call_id, 0, offset, data)
    }

    pub fn error(self, error: ErrorCode) {
        let reply = Reply::ErrorReply { error };
        if let Err(err) = self.ch.reply(self.call_id, reply) {
            println!("failed to complete read: {err:?}");
        }
    }

    pub fn complete(self, len: usize) {
        let reply = Reply::ReadReply { len };
        if let Err(err) = self.ch.reply(self.call_id, reply) {
            println!("failed to complete read: {err:?}");
        }
    }
}

pub struct WriteCompleter {
    ch: Rc<Channel>,
    call_id: CallId,
}

impl WriteCompleter {
    fn new(channel: Rc<Channel>, call_id: CallId) -> Self {
        Self {
            ch: channel,
            call_id,
        }
    }
}

impl WriteCompleter {
    pub fn read_data(&self, offset: usize, data: &mut [u8]) -> Result<usize, ErrorCode> {
        self.ch.ool_read(self.call_id, 0, offset, data)
    }

    pub fn error(self, error: ErrorCode) {
        let reply = Reply::ErrorReply { error };
        if let Err(err) = self.ch.reply(self.call_id, reply) {
            println!("failed to complete write: {err:?}");
        }
    }

    pub fn complete(self, len: usize) {
        let reply = Reply::WriteReply { len };
        if let Err(err) = self.ch.reply(self.call_id, reply) {
            println!("failed to complete write: {err:?}");
        }
    }
}

enum Object {
    Channel(Rc<Channel>),
    Interrupt(Rc<Interrupt>),
}

pub struct Context<'a> {
    sink: &'a Sink,
    objects: &'a mut HashMap<HandleId, Object>,
}

impl<'a> Context<'a> {
    fn new(sink: &'a Sink, objects: &'a mut HashMap<HandleId, Object>) -> Self {
        Self { sink, objects }
    }

    pub fn add_channel<T: Into<Rc<Channel>>>(&mut self, ch: T) -> Result<(), ErrorCode> {
        let ch = ch.into();
        self.sink.add(ch.as_ref())?;
        self.objects.insert(ch.handle().id(), Object::Channel(ch));
        Ok(())
    }

    pub fn add_interrupt<T: Into<Rc<Interrupt>>>(&mut self, interrupt: T) -> Result<(), ErrorCode> {
        let interrupt = interrupt.into();
        self.sink.add(interrupt.as_ref())?;
        self.objects
            .insert(interrupt.handle().id(), Object::Interrupt(interrupt));
        Ok(())
    }
}

pub trait Application {
    fn init(ctx: &mut Context) -> Self;

    #[allow(unused)]
    fn open(&mut self, ctx: &mut Context, completer: OpenCompleter) {
        println!("received an unexpected message: open");
        completer.error(ErrorCode::Unsupported)
    }

    #[allow(unused)]
    fn read(&mut self, ctx: &mut Context, completer: ReadCompleter, offset: usize, len: usize) {
        println!("received an unexpected message: read");
        completer.error(ErrorCode::Unsupported)
    }

    #[allow(unused)]
    fn write(&mut self, ctx: &mut Context, completer: WriteCompleter, offset: usize, len: usize) {
        println!("received an unexpected message: write");
        completer.error(ErrorCode::Unsupported)
    }

    #[allow(unused)]
    fn open_reply(&mut self, ctx: &mut Context, ch: &Rc<Channel>, uri: Buffer, new_ch: Channel) {
        println!("received an unexpected message: open reply");
    }

    #[allow(unused)]
    fn read_reply(&mut self, ctx: &mut Context, ch: &Rc<Channel>, buf: BufferMut, len: usize) {
        println!("received an unexpected message: read reply");
    }

    #[allow(unused)]
    fn write_reply(&mut self, ctx: &mut Context, ch: &Rc<Channel>, buf: Buffer, len: usize) {
        println!("received an unexpected message: write reply");
    }

    #[allow(unused)]
    fn irq(&mut self, ctx: &mut Context, interrupt: &Rc<Interrupt>, irq: u8) {
        println!("received an unexpected irq: {irq}");
    }
}

pub fn run<A: Application>() {
    let sink = Sink::new().unwrap();
    let mut objects = HashMap::new();
    let mut app = A::init(&mut Context::new(&sink, &mut objects));
    loop {
        let event = sink.wait().unwrap();
        match event {
            Event::CallMessage {
                ch_id,
                info,
                call_id,
                handles,
                inline,
            } => {
                let ch = match objects.get(&ch_id) {
                    Some(Object::Channel(ch)) => ch.clone(),
                    _ => panic!("unknown handle id from sink: {:?}", ch_id),
                };

                let mut ctx = Context::new(&sink, &mut objects);
                match info {
                    MessageInfo::OPEN => {
                        let inline = unsafe { &*(inline.as_ptr() as *const OpenInline) };
                        let completer = OpenCompleter::new(ch, call_id);
                        app.open(&mut ctx, completer);
                    }
                    MessageInfo::READ => {
                        let inline = unsafe { &*(inline.as_ptr() as *const ReadInline) };
                        let completer = ReadCompleter::new(ch, call_id);
                        app.read(&mut ctx, completer, inline.offset, inline.len);
                    }
                    MessageInfo::WRITE => {
                        let inline = unsafe { &*(inline.as_ptr() as *const WriteInline) };
                        let completer = WriteCompleter::new(ch, call_id);
                        app.write(&mut ctx, completer, inline.offset, inline.len);
                    }
                    _ => panic!("unexpected message info: {:?}", info),
                }
            }
            Event::ReplyMessage {
                ch_id,
                info,
                cookie,
                mut handles,
                inline,
            } => {
                let ch = match objects.get(&ch_id) {
                    Some(Object::Channel(ch)) => ch.clone(),
                    _ => panic!("unknown handle id from sink: {:?}", ch_id),
                };

                let mut ctx = Context::new(&sink, &mut objects);
                match info {
                    MessageInfo::OPEN_REPLY => {
                        let inline = unsafe { &*(inline.as_ptr() as *const OpenReplyInline) };
                        let Cookie::Buffer(uri) = *cookie else {
                            panic!("unexpected cookie type");
                        };
                        let new_ch = Channel::from_handle(handles.pop().unwrap());
                        app.open_reply(&mut ctx, &ch, uri, new_ch);
                    }
                    MessageInfo::READ_REPLY => {
                        let inline = unsafe { &*(inline.as_ptr() as *const ReadReplyInline) };
                        let Cookie::BufferMut(buf) = *cookie else {
                            panic!("unexpected cookie type");
                        };
                        app.read_reply(&mut ctx, &ch, buf, inline.len);
                    }
                    MessageInfo::WRITE_REPLY => {
                        let inline = unsafe { &*(inline.as_ptr() as *const WriteReplyInline) };
                        let Cookie::Buffer(buf) = *cookie else {
                            panic!("unexpected cookie type");
                        };
                        app.write_reply(&mut ctx, &ch, buf, inline.len);
                    }
                    _ => panic!("unexpected message info: {:?}", info),
                }
            }
            Event::Irq { handle_id, irq } => {
                let interrupt = match objects.get(&handle_id) {
                    Some(Object::Interrupt(interrupt)) => interrupt.clone(),
                    _ => panic!("unknown handle id from sink: {:?}", handle_id),
                };

                let mut ctx = Context::new(&sink, &mut objects);
                app.irq(&mut ctx, &interrupt, irq);
            }
        }
    }
}
