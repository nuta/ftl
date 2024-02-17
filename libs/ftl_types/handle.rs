use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize, Serialize)]
pub struct HandleId(isize);

impl HandleId {
    pub fn new(id: isize) -> Self {
        Self(id)
    }

    pub fn as_isize(&self) -> isize {
        self.0
    }
}
