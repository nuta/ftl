use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct Spec<'a> {
    #[serde(flatten, borrow)]
    pub spec: SpecKind<'a>,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum SpecKind<'a> {
    Fiber(#[serde(borrow)] FiberSpec<'a>),
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct FiberSpec<'a> {
    pub name: &'a str,
    pub deps: &'a [&'a str],
}
