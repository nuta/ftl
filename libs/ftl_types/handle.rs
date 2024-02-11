use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize, Serialize)]
pub struct HandleId(isize);
