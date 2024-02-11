use alloc::{collections::BTreeMap, string::String};
use serde::{Deserialize, Serialize};

use crate::handle::HandleId;

#[derive(Debug, Deserialize, Serialize)]
pub struct Environ {
    pub deps: BTreeMap<String, HandleId>,
}
