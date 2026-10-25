use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl::quota::Quota;
use ftl::vmo::Vmo;
use ftl_utils::spinlock::SpinLock;

use crate::net::TcpIp;
use crate::types::c_short;
use crate::types::errno::Errno;
use crate::types::sys::poll::POLLOUT;
use crate::vfs::FileLike;
use crate::wait_queue::Sleep;
use crate::zone::Zone;
use crate::zone::spec::SpecParser;

/// The global state of the zone system.
struct Globals {
    quota: Arc<Quota>,
    image: Vmo,
    network: Option<Arc<TcpIp>>,
    zones: SpinLock<Vec<Zone>>,
}

pub struct Fork {
    globals: Arc<Globals>,
}

impl Fork {
    pub fn new(quota: Arc<Quota>, image: Vmo, network: Option<Arc<TcpIp>>) -> Self {
        Self {
            globals: Arc::new(Globals {
                quota,
                image,
                network,
                zones: SpinLock::new(Vec::new()),
            }),
        }
    }
}

impl FileLike for Fork {
    fn open(&self) -> Result<Option<Arc<dyn FileLike>>, Errno> {
        Ok(Some(Arc::new(ForkBuilder {
            globals: self.globals.clone(),
            state: SpinLock::new(State::Parse(SpecParser::new())),
        })))
    }
}

enum State {
    Parse(SpecParser),
    Done,
}

struct ForkBuilder {
    globals: Arc<Globals>,
    state: SpinLock<State>,
}

impl FileLike for ForkBuilder {
    fn write(
        &self,
        buf: &[u8],
        _offset: usize,
        _nonblocking: bool,
        _sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let mut state = self.state.lock();
        let State::Parse(parser) = &mut *state else {
            // You can't write to a finished spec.
            return Err(Errno::EINVAL);
        };

        match parser.feed(buf) {
            Ok(None) => Ok(buf.len()),
            Ok(Some(spec)) => {
                *state = State::Done;

                // Start a new zone.
                let zone = Zone::spawn(
                    &self.globals.quota,
                    &self.globals.image,
                    self.globals.network.as_ref(),
                    &spec,
                )?;
                self.globals.zones.lock().push(zone);
                Ok(buf.len())
            }
            Err(err) => {
                // An error occurred while parsing the spec.
                *state = State::Done;
                Err(err)
            }
        }
    }

    fn poll(&self) -> Result<c_short, Errno> {
        Ok(POLLOUT)
    }
}
