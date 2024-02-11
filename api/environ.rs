pub struct Environ {
    raw: ftl_types::environ::Environ,
}

impl Environ {
    pub fn from_raw(raw: ftl_types::environ::Environ) -> Self {
        Self { raw }
    }

    pub fn parse_deps<Deps>(&self) -> Deps {
        todo!()
    }
}
