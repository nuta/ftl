use crate::{
    channel::{Message, SendError},
    Handle,
};

pub fn channel_create() -> crate::Result<Handle> {
    todo!()
}

pub fn channel_send(handle: Handle, message: Message) -> Result<(), SendError> {
    todo!()
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
