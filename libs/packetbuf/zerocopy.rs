use core::fmt;

use num_enum::TryFromPrimitive;
use zerocopy::{network_endian::U16, AsBytes, FromBytes, FromZeroes};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct MacAddr([u8; 6]);

impl MacAddr {
    pub const BROADCAST: MacAddr = MacAddr([0xff; 6]);

    pub const fn new(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8) -> MacAddr {
        MacAddr([a, b, c, d, e, f])
    }

    pub const fn from_bytes(bytes: [u8; 6]) -> MacAddr {
        MacAddr(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 6] {
        self.0
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5],
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Ipv4Addr(u32);

impl Ipv4Addr {
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        Ipv4Addr(u32::from_ne_bytes([a, b, c, d]))
    }

    pub const fn from_ne_bytes(bytes: [u8; 4]) -> Ipv4Addr {
        Ipv4Addr(u32::from_ne_bytes(bytes))
    }

    pub const fn as_ne_bytes(self) -> [u8; 4] {
        self.0.to_ne_bytes()
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.0 >> 24,
            (self.0 >> 16) & 0xff,
            self.0 >> 8 & 0xff,
            self.0 & 0xff
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive)]
#[repr(u16)]
pub enum EtherType {
    Arp = 0x0806,
    Ipv4 = 0x0800,
}

#[derive(Debug, FromZeroes, FromBytes, AsBytes)]
#[repr(C)]
pub struct EthernetHeader {
    pub dst: [u8; 6],
    pub src: [u8; 6],
    pub ethertype: U16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive)]
#[repr(u16)]
pub enum ArpOp {
    Request = 1,
    Reply = 2,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive)]
#[repr(u16)]
pub enum ArpHwType {
    Ethernet = 1,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive)]
#[repr(u16)]
pub enum ArpProtoType {
    Ipv4 = 0x0800,
}

#[derive(FromZeroes, FromBytes, AsBytes)]
#[repr(C)]
pub struct ArpPacket {
    pub hw_type: U16,
    pub proto_type: U16,
    pub hw_addr_len: u8,
    pub proto_addr_len: u8,
    pub op: U16,
    pub src_hw_addr: [u8; 6],
    pub src_proto_addr: [u8; 4],
    pub dst_hw_addr: [u8; 6],
    pub dst_proto_addr: [u8; 4],
}
