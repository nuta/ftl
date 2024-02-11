use ftl_types::handle::HandleId;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Handle {
    raw: HandleId,
}

impl Handle {
    // TODO: make unsafe
    pub fn new(raw: HandleId) -> Self {
        Self { raw }
    }

    pub fn id(&self) -> HandleId {
        self.raw
    }
}
