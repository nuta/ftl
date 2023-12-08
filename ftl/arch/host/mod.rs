use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::{
    channel::{Message, SendError},
    Error, Handle,
};

pub struct Printer;

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        use std::io::{stderr, Write};
        stderr().write_all(s.as_bytes()).unwrap();

        Ok(())
    }
}

#[macro_export]
macro_rules! log {
    ($level:expr, $message:literal) => {{
        use core::fmt::Write;
        let _ = write!($crate::arch::Printer, "{}\n", $message);
    }};
    ($level:expr, $format:literal, $($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::arch::Printer, concat!($format, "\n"), $($arg)*);
    }};
}

static HANDLES: Lazy<Mutex<HandleTable>> = Lazy::new(|| Mutex::new(HandleTable::new()));

struct HandleTable {
    next_id: AtomicU32,
    handles: HashMap<Handle, Arc<Object>>,
}

impl HandleTable {
    pub fn new() -> HandleTable {
        HandleTable {
            next_id: AtomicU32::new(1),
            handles: HashMap::new(),
        }
    }

    pub fn insert(&mut self, object: Arc<Object>) -> Handle {
        let handle = Handle::from_raw(self.next_id.fetch_add(1, Ordering::SeqCst));
        self.handles.insert(handle, object);
        handle
    }

    pub fn get_as_channel(&self, handle: Handle) -> crate::Result<&Arc<Mutex<Channel>>> {
        self.handles
            .get(&handle)
            .ok_or(Error::HandleNotFound)?
            .as_channel()
    }
}

pub enum Object {
    Channel(Arc<Mutex<Channel>>),
    Other,
}

impl Object {
    pub fn as_channel(&self) -> crate::Result<&Arc<Mutex<Channel>>> {
        match self {
            Object::Channel(ch) => Ok(ch),
            _ => Err(Error::HandleTypeMismatch)?,
        }
    }
}

pub struct Channel {
    rx: VecDeque<Message>,
    peer: Option<Arc<Mutex<Channel>>>,
}

pub fn channel_create() -> crate::Result<(Handle, Handle)> {
    let ch1 = Arc::new(Mutex::new(Channel {
        rx: VecDeque::new(),
        peer: None,
    }));

    let ch2 = Arc::new(Mutex::new(Channel {
        rx: VecDeque::new(),
        peer: None,
    }));

    ch1.lock().peer = Some(ch2.clone());
    ch2.lock().peer = Some(ch1.clone());

    let mut handles = HANDLES.lock();
    let handle1 = handles.insert(Arc::new(Object::Channel(ch1)));
    let handle2 = handles.insert(Arc::new(Object::Channel(ch2)));

    Ok((handle1, handle2))
}

pub fn channel_send(handle: Handle, message: Message) -> Result<(), SendError> {
    let handles = HANDLES.lock();
    let ch = handles
        .get_as_channel(handle)
        .map_err(SendError::Error)?
        .lock();

    match ch.peer {
        Some(ref peer_ch) => {
            peer_ch.lock().rx.push_back(message);
        }
        None => {
            return Err(SendError::Error(crate::Error::ClosedByPeer));
        }
    }

    Ok(())
}

pub fn channel_recv(handle: Handle) -> crate::Result<Option<Message>> {
    let handles = HANDLES.lock();
    let mut ch = handles.get_as_channel(handle)?.lock();
    Ok(ch.rx.pop_front())
}

pub fn channel_call(handle: Handle, message: Message) -> crate::Result<Message> {
    let handles = HANDLES.lock();
    let mut ch = handles.get_as_channel(handle)?.lock();

    todo!()
}

pub fn channel_close(handle: Handle) -> crate::Result<()> {
    todo!()
}
