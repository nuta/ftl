use ftl_types::{error::FtlError, message::MessageInfo};

use crate::ref_counted::SharedRef;

pub struct Channel {
}

impl Channel {
    pub fn new() -> Result<(SharedRef<Channel>, SharedRef<Channel>), FtlError> {
        todo!()
    }

    pub fn send(&self, msginfo: MessageInfo, data: &[u8]) -> Result<(), FtlError> {
        todo!()
    }
}
