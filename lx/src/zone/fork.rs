use alloc::sync::Arc;
use alloc::vec::Vec;
use core::net::Ipv4Addr;

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

const CHILD_IP_START: Ipv4Addr = Ipv4Addr::new(10, 77, 0, 2);
const CHILD_IP_END: Ipv4Addr = Ipv4Addr::new(10, 77, 255, 254);

struct ChildIpAllocator {
    used_ips: SpinLock<Vec<Ipv4Addr>>,
}

impl ChildIpAllocator {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            used_ips: SpinLock::new(Vec::new()),
        })
    }

    fn alloc(self: &Arc<Self>) -> Result<ChildIp, Errno> {
        let mut used_ips = self.used_ips.lock();
        for bits in CHILD_IP_START.to_bits()..=CHILD_IP_END.to_bits() {
            let ip = Ipv4Addr::from_bits(bits);
            if !used_ips.contains(&ip) {
                // No zone is using this IP.
                used_ips.push(ip);
                return Ok(ChildIp {
                    allocator: self.clone(),
                    ip,
                });
            }
        }

        Err(Errno::EADDRNOTAVAIL)
    }

    fn free(&self, ip: Ipv4Addr) {
        self.used_ips.lock().retain(|used| *used != ip);
    }
}

struct ChildIp {
    allocator: Arc<ChildIpAllocator>,
    ip: Ipv4Addr,
}

impl ChildIp {
    fn ip(&self) -> Ipv4Addr {
        self.ip
    }
}

impl Drop for ChildIp {
    fn drop(&mut self) {
        self.allocator.free(self.ip);
    }
}

/// The global state of the zone system.
struct Globals {
    quota: Arc<Quota>,
    image: Vmo,
    network: Arc<TcpIp>,
    child_ips: Arc<ChildIpAllocator>,
}

pub struct Fork {
    globals: Arc<Globals>,
}

impl Fork {
    pub fn new(quota: Arc<Quota>, image: Vmo, network: Arc<TcpIp>) -> Self {
        Self {
            globals: Arc::new(Globals {
                quota,
                image,
                network,
                child_ips: ChildIpAllocator::new(),
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
    Running { _zone: Zone, _child_ip: ChildIp },
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
                let child_ip = self.globals.child_ips.alloc()?;
                let zone = Zone::spawn(
                    &self.globals.quota,
                    &self.globals.image,
                    &self.globals.network,
                    &spec,
                    child_ip.ip(),
                )?;

                *state = State::Running {
                    _zone: zone,
                    _child_ip: child_ip,
                };
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
