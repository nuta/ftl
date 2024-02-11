use ftl_types::handle::HandleId;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Handle {
    raw: HandleId,
}

impl Handle {
    pub fn id(&self) -> HandleId {
        self.raw
    }
}
