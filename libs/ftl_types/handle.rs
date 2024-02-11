use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize, Serialize)]
pub struct HandleId(isize);

impl HandleId {
    pub fn new(id: isize) -> Self {
        Self(id)
    }

    pub fn id(&self) -> isize {
        self.0
    }
}
