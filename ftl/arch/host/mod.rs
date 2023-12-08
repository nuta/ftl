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

static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);
static HANDLES: Lazy<Mutex<HashMap<Handle, Arc<Object>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn alloc_handle_id() -> Handle {
    Handle::from_raw(NEXT_HANDLE.fetch_add(1, Ordering::SeqCst))
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
    let handle1 = alloc_handle_id();
    let handle2 = alloc_handle_id();

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
    handles.insert(handle1, Arc::new(Object::Channel(ch1)));
    handles.insert(handle1, Arc::new(Object::Channel(ch2)));

    Ok((handle1, handle2))
}

pub fn channel_send(handle: Handle, message: Message) -> Result<(), SendError> {
    let handles = HANDLES.lock();
    let mut self_ch = handles
        .get(&handle)
        .ok_or(SendError::Error(Error::HandleNotFound))?
        .as_channel()
        .map_err(SendError::Error)?
        .lock();

    match self_ch.peer {
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
    todo!()
}

pub fn channel_call(handle: Handle, message: Message) -> crate::Result<Message> {
    todo!()
}

pub fn channel_close(handle: Handle) -> crate::Result<()> {
    todo!()
}
