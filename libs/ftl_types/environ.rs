use core::fmt;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::handle::HandleId;

#[derive(Deserialize, Serialize)]
pub struct Device {
    pub name: String,
    pub compatible: String,
    pub reg: u64,
    pub interrupts: Option<Vec<u32>>,
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Device")
            .field("name", &self.name)
            .field("compatible", &self.compatible)
            .field("reg", &format_args!("{:#08x}", self.reg))
            .field("interrupts", &self.interrupts)
            .finish()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Environ {
    pub deps: BTreeMap<String, HandleId>,
    pub device: Option<Device>,
}
