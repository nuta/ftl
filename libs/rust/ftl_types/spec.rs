use alloc::string::String;
use alloc::vec::Vec;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecFile {
    pub name: String,
    pub spec: Spec,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Spec {
    #[serde(rename = "app/v0")]
    App(AppSpec),
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSpec {
    pub deps: Vec<String>,
    pub provides: Vec<String>,
}
