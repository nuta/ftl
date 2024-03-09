use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::collections::Vec;

#[derive(Debug)]
pub enum ParseDepsError {
    AlreadyTaken,
    SerdeError(serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct EnvironJson {
    pub deps: serde_json::Value,
    devices: Option<Vec<ftl_types::environ::Device>>,
}

pub struct Environ {
    deps: Option<serde_json::Value>,
    devices: Option<Vec<ftl_types::environ::Device>>,
}

impl Environ {
    pub fn from_str(s: &str) -> Self {
        let json: EnvironJson = serde_json::from_str(s).expect("environ is not valid json");
        Self {
            deps: Some(json.deps),
            devices: json.devices,
        }
    }

    pub fn parse_deps<Deps: DeserializeOwned>(&mut self) -> Result<Deps, ParseDepsError> {
        let deps_value = self.deps.take().ok_or(ParseDepsError::AlreadyTaken)?;
        let deps = serde_json::from_value(deps_value).map_err(ParseDepsError::SerdeError)?;
        Ok(deps)
    }

    pub fn devices(&self) -> Option<&[ftl_types::environ::Device]> {
        self.devices.as_ref().map(Vec::as_slice)
    }
}
