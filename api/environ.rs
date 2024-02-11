use serde::{de::DeserializeOwned, Deserialize};

#[derive(Debug)]
pub enum ParseDepsError {
    AlreadyTaken,
    SerdeError(serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct EnvironJson {
    pub deps: serde_json::Value,
}

pub struct Environ {
    deps: Option<serde_json::Value>,
}

impl Environ {
    pub fn from_str(s: &str) -> Self {
        let json: EnvironJson = serde_json::from_str(s).expect("environ is not valid json");
        Self {
            deps: Some(json.deps),
        }
    }

    pub fn parse_deps<Deps: DeserializeOwned>(&mut self) -> Result<Deps, ParseDepsError> {
        let deps_value = self.deps.take().ok_or(ParseDepsError::AlreadyTaken)?;
        let deps = serde_json::from_value(deps_value).map_err(ParseDepsError::SerdeError)?;
        Ok(deps)
    }
}
