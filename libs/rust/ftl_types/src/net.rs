use core::num::NonZeroU16;
use core::num::NonZeroU32;

pub const IP_VERSION_4: u8 = 4;
pub const IP_PROTOCOL_TCP: u8 = 6;
pub const NET_MAX_HEADER_LEN: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Rule {
    ip_version: u8,
    ip_protocol: u8,
    local_port: Option<NonZeroU16>,
    local_ip: Option<NonZeroU32>,
    remote_ip: Option<NonZeroU32>,
    remote_port: Option<NonZeroU16>,
    _padding: u16,
}

impl Rule {
    pub const fn tcp_ipv4_listener(local_ip: Option<NonZeroU32>, local_port: NonZeroU16) -> Self {
        Self {
            ip_version: IP_VERSION_4,
            ip_protocol: IP_PROTOCOL_TCP,
            local_port: Some(local_port),
            local_ip,
            remote_ip: None,
            remote_port: None,
            _padding: 0,
        }
    }

    pub const fn tcp_ipv4_flow(
        local_ip: NonZeroU32,
        local_port: NonZeroU16,
        remote_ip: NonZeroU32,
        remote_port: NonZeroU16,
    ) -> Self {
        Self {
            ip_version: IP_VERSION_4,
            ip_protocol: IP_PROTOCOL_TCP,
            local_port: Some(local_port),
            local_ip: Some(local_ip),
            remote_ip: Some(remote_ip),
            remote_port: Some(remote_port),
            _padding: 0,
        }
    }

    pub const fn is_supported(&self) -> bool {
        self.ip_version == IP_VERSION_4
            && self.ip_protocol == IP_PROTOCOL_TCP
            && self.local_port.is_some()
            && self.remote_ip.is_some() == self.remote_port.is_some()
    }

    pub const fn matches(
        &self,
        local_ip: u32,
        local_port: u16,
        remote_ip: u32,
        remote_port: u16,
    ) -> bool {
        matches_optional_u32(self.local_ip, local_ip)
            && matches_optional_u32(self.remote_ip, remote_ip)
            && matches_optional_u16(self.local_port, local_port)
            && matches_optional_u16(self.remote_port, remote_port)
    }

    pub const fn specificity(&self) -> u32 {
        self.local_ip.is_some() as u32
            + self.remote_ip.is_some() as u32
            + self.local_port.is_some() as u32
            + self.remote_port.is_some() as u32
    }
}

const fn matches_optional_u16(expected: Option<NonZeroU16>, actual: u16) -> bool {
    match expected {
        Some(expected) => expected.get() == actual,
        None => true,
    }
}

const fn matches_optional_u32(expected: Option<NonZeroU32>, actual: u32) -> bool {
    match expected {
        Some(expected) => expected.get() == actual,
        None => true,
    }
}
