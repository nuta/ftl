use core::num::NonZeroU16;
use core::num::NonZeroU32;

pub const ETHTYPE_IPV4: u16 = 0x0800;
pub const ETHTYPE_ARP: u16 = 0x0806;
pub const IPPROTO_TCP: u16 = 0x06;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Rule {
    eth_type: u16, // eth type
    ip_proto: u16, // protocol in IPv4 header
    local_ip: Option<NonZeroU32>,
    remote_ip: Option<NonZeroU32>,
    local_port: Option<NonZeroU16>,
    remote_port: Option<NonZeroU16>,
}

impl Rule {
    pub const fn new(
        eth_type: u16,
        ip_proto: u16,
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

    /// Returns `Some(specificity)` if the rule matches.
    ///
    /// Because a network has multiple rules, like CSS selectors, multiple rules
    /// may match the same packet. The specificity is a number to compare the
    /// priority of the rules. Higher is more specific, and is non-zero.
    pub const fn matches(
        &self,
        eth_type: u16,
        ip_proto: u16,
        local_ip: u32,
        local_port: u16,
        remote_ip: u32,
        remote_port: u16,
    ) -> Option<u8> {
        if self.eth_type != eth_type {
            return None;
        }

        if self.ip_proto != ip_proto {
            return None;
        }

        let mut specificity = 1;
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
