use ftl_types::handle::HandleId;

pub struct OwnedHandle(HandleId);

impl OwnedHandle {
    pub(crate) fn from_raw(id: HandleId) -> Self {
        Self(id)
    }

    pub const fn id(&self) -> HandleId {
        self.0
    }
}

pub trait Handleable {
    fn handle(&self) -> &OwnedHandle;
}
