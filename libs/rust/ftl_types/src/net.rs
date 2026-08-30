use core::num::NonZeroU16;
use core::num::NonZeroU32;

pub const IP_VERSION_4: u8 = 4;
pub const IP_PROTOCOL_TCP: u8 = 6;
pub const TCP_LISTEN: u8 = 0;
pub const TCP_CONNECT: u8 = 1;
pub const NET_MAX_HEADER_LEN: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Rule {
    ip_proto: u8,
    transport_proto: u8,
    local_ip: Option<NonZeroU32>,
    remote_ip: Option<NonZeroU32>,
    local_port: Option<NonZeroU16>,
    remote_port: Option<NonZeroU16>,
}

impl Rule {
    pub const fn new(
        ip_proto: u8,
        transport_proto: u8,
        local_ip: Option<NonZeroU32>,
        local_port: Option<NonZeroU16>,
        remote_ip: Option<NonZeroU32>,
        remote_port: Option<NonZeroU16>,
    ) -> Self {
        Self {
            ip_proto,
            transport_proto,
            local_ip,
            local_port,
            remote_ip,
            remote_port,
        }
    }

    pub const fn is_supported(&self) -> bool {
        if self.ip_proto != IP_PROTOCOL_TCP {
            return false;
        }

        match self.transport_proto {
            TCP_LISTEN => {
                self.local_port.is_some() && self.remote_ip.is_none() && self.remote_port.is_none()
            }
            TCP_CONNECT => {
                self.local_ip.is_some()
                    && self.local_port.is_some()
                    && self.remote_ip.is_some()
                    && self.remote_port.is_some()
            }
            _ => false,
        }
    }

    /// Returns `Some(specificity)` if the rule matches.
    ///
    /// Because a network has multiple rules, like CSS selectors, multiple rules
    /// may match the same packet. The specificity is a number to compare the
    /// priority of the rules. Higher is more specific.
    pub const fn matches(
        &self,
        local_ip: u32,
        local_port: u16,
        remote_ip: u32,
        remote_port: u16,
    ) -> Option<u8> {
        let mut specificity = 0;
        if let Some(expected) = self.local_ip {
            if expected.get() != local_ip {
                return None;
            }

            specificity += 1;
        }

        if let Some(expected) = self.remote_ip {
            if expected.get() != remote_ip {
                return None;
            }

            specificity += 1;
        }

        if let Some(expected) = self.local_port {
            if expected.get() != local_port {
                return None;
            }

            specificity += 1;
        }

        if let Some(expected) = self.remote_port {
            if expected.get() != remote_port {
                return None;
            }

            specificity += 1;
        }

        Some(specificity)
    }
}
