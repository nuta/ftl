use alloc::vec::Vec;

use ftl_api::start::StartInfo;
use ftl_utils::spinlock::SpinLock;

use crate::arch;
use crate::boot::BootInfo;
use crate::initfs;
use crate::loader::LoadedElf;

const START_INFO: &StartInfo = &StartInfo {
    print: |bytes| {
        arch::console_write(bytes);
    },
    panic: || {
        panic!("server panicked");
    },
};

static SERVERS: SpinLock<Vec<Server>> = SpinLock::new(Vec::new());

pub struct Server {
    image: *const u8,
}

impl Server {
    fn load(elf_file: &[u8]) -> Result<Self, crate::loader::Error> {
        let LoadedElf { image, entry_fn } = crate::loader::load_elf(elf_file)?;
        entry_fn(START_INFO);
        Ok(Self { image })
    }
}

unsafe impl Send for Server {}

pub fn init(bootinfo: &BootInfo) {
    for module in &bootinfo.modules {
        let initfs = initfs::InitFsLoader::new(module);
        for file in initfs {
            if file.name.starts_with(b"servers/") && file.name.ends_with(b".elf") {
                let name = core::str::from_utf8(file.name).unwrap();
                trace!("loading {}...", name);
                match Server::load(file.data) {
                    Ok(server) => {
                        SERVERS.lock().push(server);
                    }
                    Err(e) => {
                        error!("failed to load server: {:?}", e);
                    }
                }
                trace!("loaded {}", name);
            }
        }
    }
}
