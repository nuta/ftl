use ftl_types::error::ErrorCode;

pub struct EventLoop {}

impl EventLoop {
    pub fn new() -> Result<Self, ErrorCode> {
        Ok(Self {})
    }
}
