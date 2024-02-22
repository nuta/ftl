use alloc::string::String;
use alloc::vec::Vec;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct Spec {
    #[serde(flatten)]
    pub spec: SpecKind,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", content = "spec")]
pub enum SpecKind {
    #[serde(rename = "fiber/v0")]
    Fiber(FiberSpec),
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct DeviceTreeEntry {
    pub compatible: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct FiberSpec {
    pub name: String,
    pub deps: Vec<String>,
    pub provides: Vec<String>,
    pub device_tree: Option<Vec<DeviceTreeEntry>>,
}
