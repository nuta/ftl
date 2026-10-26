pub const ETHTYPE_IPV4: u16 = 0x0800;
pub const ETHTYPE_ARP: u16 = 0x0806;
pub const IP_PROTO_TCP: u8 = 0x06;
pub const IP_PROTO_UDP: u8 = 0x11;

pub const PROTO_IPV4: u8 = 1 << 0;
// TODO: pub const PROTO_IPV6: u8 = 1 << 1;
pub const PROTO_TCP: u8 = 1 << 2;
pub const PROTO_UDP: u8 = 1 << 3;
pub const PROTO_ANY: u8 = PROTO_IP | PROTO_TRANSPORT;
const PROTO_IP: u8 = PROTO_IPV4;
const PROTO_TRANSPORT: u8 = PROTO_TCP | PROTO_UDP;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct RouteRule {
    pub protocols: u8,
    pub prefix_len: u8,
    pub port_min: u16,
    pub port_max: u16,
    pub ip: u32,
    pub src_ip: u32,
}

impl RouteRule {
    pub const fn new(protocols: u8, port_min: u16, port_max: u16) -> Self {
        Self {
            protocols,
            prefix_len: 0,
            port_min,
            port_max,
            ip: 0,
            src_ip: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.protocols & !PROTO_ANY == 0
            && self.protocols & PROTO_IP != 0
            && self.protocols & PROTO_TRANSPORT != 0
            && self.port_min <= self.port_max
            && self.prefix_len <= 32
            && self.ip & self.netmask() == self.ip
    }

    pub const fn netmask(&self) -> u32 {
        match u32::MAX.checked_shl(32 - self.prefix_len as u32) {
            Some(netmask) => netmask,
            None => 0,
        }
    }

    pub fn matches(&self, protocols: u8, ip: u32, port: u16) -> bool {
        self.protocols & protocols == protocols
            && ip & self.netmask() == self.ip
            && self.port_min <= port
            && port <= self.port_max
    }

    pub fn overlaps(&self, other: &RouteRule) -> bool {
        let protocols = self.protocols & other.protocols;
        protocols & PROTO_IP != 0
            && protocols & PROTO_TRANSPORT != 0
            && self.ip == other.ip
            && self.prefix_len == other.prefix_len
            && other.port_min <= self.port_max
            && self.port_min <= other.port_max
    }
}
