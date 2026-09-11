use core::num::NonZeroU16;
use core::num::NonZeroU32;

pub const ETHTYPE_IPV4: u16 = 0x0800;
pub const ETHTYPE_ARP: u16 = 0x0806;
pub const IPPROTO_TCP: u8 = 0x06;
pub const IPPROTO_UDP: u8 = 0x11;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Rule {
    eth_type: u16, // eth type
    ip_proto: u8,  // protocol in IPv4 header
    local_ip: Option<NonZeroU32>,
    remote_ip: Option<NonZeroU32>,
    local_port: Option<NonZeroU16>,
    remote_port: Option<NonZeroU16>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct FiveTuple {
    pub eth_type: u16,
    pub ip_proto: u8,
    pub local_ip: u32,
    pub local_port: u16,
    pub remote_ip: u32,
    pub remote_port: u16,
}

impl FiveTuple {
    pub fn matchers(self) -> FiveTupleMatchers {
        FiveTupleMatchers {
            tuple: self,
            state: MatcherState::Exact,
        }
    }
}

enum MatcherState {
    Exact,
    LocalPort,
    Done,
}

/// An iterator that yields rules that matches the five tuple.
pub struct FiveTupleMatchers {
    tuple: FiveTuple,
    state: MatcherState,
}

impl Iterator for FiveTupleMatchers {
    type Item = Rule;

    fn next(&mut self) -> Option<Rule> {
        let remote_ip = NonZeroU32::new(self.tuple.remote_ip);
        let remote_port = NonZeroU16::new(self.tuple.remote_port);
        let (next, remote_ip, remote_port) = match self.state {
            MatcherState::Exact => (MatcherState::LocalPort, remote_ip, remote_port),
            MatcherState::LocalPort => (MatcherState::Done, None, None),
            MatcherState::Done => return None,
        };

        // The local port must be already matched.
        let local_port = NonZeroU16::new(self.tuple.local_port)?;

        self.state = next;
        Some(Rule::new(
            self.tuple.eth_type,
            self.tuple.ip_proto,
            // TODO: binding to a specific local IP
            None,
            Some(local_port),
            remote_ip,
            remote_port,
        ))
    }
}

impl Rule {
    pub const fn new(
        eth_type: u16,
        ip_proto: u8,
        local_ip: Option<NonZeroU32>,
        local_port: Option<NonZeroU16>,
        remote_ip: Option<NonZeroU32>,
        remote_port: Option<NonZeroU16>,
    ) -> Self {
        Self {
            eth_type,
            ip_proto,
            local_ip,
            local_port,
            remote_ip,
            remote_port,
        }
    }

    pub const fn local_port(&self) -> Option<NonZeroU16> {
        self.local_port
    }
}
