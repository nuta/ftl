use alloc::string::String;
use alloc::vec::Vec;

use ftl_types::spec::AppSpec;
use hashbrown::HashMap;

use crate::channel::Channel;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AppName(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ProtocolName(String);

struct App {
    name: String,
    spec: AppSpec,
    elf_file: &'static [u8],
    their_ch: Option<Channel>,
    our_ch: Channel,
}

pub struct Autopilot {
    providers: HashMap<ProtocolName, AppName>,
    apps: Vec<App>,
}

impl Autopilot {
    pub fn start_apps(&mut self) {
        for app in &self.apps {
            for name in &app.spec.provides {
                self.providers
                    .insert(ProtocolName(name.clone()), AppName(app.name.clone()));
            }
        }
    }
}
