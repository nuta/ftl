use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use once_cell::sync::Lazy;
use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};

use crate::{
    channel::{Message, SendError},
    Handle,
};

static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);
static HANDLES: Lazy<Mutex<HashMap<Handle, Arc<Mutex<Object>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn open_channel<'a>(
    handles: &'a MutexGuard<'a, HashMap<Handle, Arc<Mutex<Object>>>>,
    handle: Handle,
) -> crate::Result<MappedMutexGuard<'a, RawChannel>> {
    let object_lock = handles
        .get(&handle)
        .ok_or(crate::Error::HandleNotFound)?
        .lock();

    if matches!(&*object_lock, Object::Channel(_)) {
        return Err(crate::Error::HandleTypeMismatch);
    }

    Ok(MutexGuard::map(object_lock, |object| match object {
        Object::Channel(ch) => ch,
        _ => unreachable!(),
    }))
}

fn alloc_handle_id() -> Handle {
    Handle::from_raw(NEXT_HANDLE.fetch_add(1, Ordering::SeqCst))
}

pub enum Object {
    Channel(RawChannel),
    Other,
}

pub struct RawChannel {
    rx: VecDeque<Message>,
    peer: Option<Arc<Mutex<RawChannel>>>,
}

pub fn channel_create() -> crate::Result<Handle> {
    let handle = alloc_handle_id();
    HANDLES.lock().insert(
        handle,
        Arc::new(Mutex::new(Object::Channel(RawChannel {
            rx: VecDeque::new(),
            peer: None,
        }))),
    );
    Ok(handle)
}

pub fn channel_send(handle: Handle, message: Message) -> Result<(), SendError> {
    let handles = HANDLES.lock();
    let mut self_ch = open_channel(&handles, handle).map_err(SendError::Error)?;
    match self_ch.peer {
        Some(ref peer_ch) => {
            peer_ch.lock().rx.push_back(message);
        }
        None => {
            self_ch.rx.push_back(message);
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
