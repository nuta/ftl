use alloc::string::String;
use alloc::vec::Vec;

use ftl_autogen::protocols::NewclientRequest;
use ftl_types::error::FtlError;
use ftl_types::handle::HandleRights;
use ftl_types::message::MessageBody;
use ftl_types::message::MessageBuffer;
use ftl_types::spec::AppSpec;
use hashbrown::HashMap;

use crate::app_loader::AppLoader;
use crate::channel::Channel;
use crate::handle::Handle;
use crate::ref_counted::SharedRef;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppName(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolName(String);

struct App {
    name: String,
    spec: AppSpec,
    elf_file: &'static [u8],
    their_ch: Option<SharedRef<Channel>>,
    our_ch: SharedRef<Channel>,
}

#[derive(Debug)]
pub enum Error {
    CreateChannel(FtlError),
    SendMessage(FtlError),
    NoService {
        app: AppName,
        protocol: ProtocolName,
    },
}

pub struct Autopilot {
    providers: HashMap<ProtocolName, AppName>,
    apps: HashMap<AppName, App>,
}

impl Autopilot {
    pub fn start_apps(&mut self, apps: Vec<(String, AppSpec, &'static [u8])>) -> Result<(), Error> {
        for (name, spec, elf_file) in apps {
            let (their_ch, our_ch) = Channel::new().map_err(Error::CreateChannel)?;
            self.apps.insert(
                AppName(name.clone()),
                App {
                    name: name,
                    spec: spec,
                    elf_file,
                    their_ch: Some(their_ch),
                    our_ch,
                },
            );
        }

        for app in self.apps.values() {
            for name in &app.spec.provides {
                self.providers
                    .insert(ProtocolName(name.clone()), AppName(app.name.clone()));
            }
        }

        let mut msgbuffer = MessageBuffer::new();
        for app in self.apps.values() {
            let mut depend_handles = Vec::new();
            for name in &app.spec.depends {
                let proto_name = ProtocolName(name.clone());
                let (provider_ch, app_ch) = Channel::new().map_err(Error::CreateChannel)?;
                let provider_name = match self.providers.get(&proto_name) {
                    Some(name) => name,
                    None => {
                        return Err(Error::NoService {
                            app: AppName(app.name.clone()),
                            protocol: proto_name,
                        });
                    }
                };

                let provider = self.apps.get(provider_name).unwrap();

                provider
                    .our_ch
                    .send(NewclientRequest::MSGINFO, &msgbuffer)
                    .map_err(Error::SendMessage)?;

                depend_handles.push((
                    proto_name.0,
                    Handle::new(app_ch.into(), HandleRights::NONE).into(),
                ));
            }

            AppLoader::parse(app.elf_file)
                .expect("invalid ELF")
                .load(alloc::vec![], depend_handles)
                .expect("failed to load ELF");
        }

        Ok(())
    }
}
