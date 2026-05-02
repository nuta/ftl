use core::fmt;
use core::ops::BitOr;
use core::ops::BitOrAssign;

use crate::endian::Ne;
use crate::packet::WriteableToPacket;

#[repr(C, packed)]
pub(super) struct TcpHeader {
    pub src_port: Ne<u16>,
    pub dst_port: Ne<u16>,
    pub seq: Ne<u32>,
    pub ack: Ne<u32>,
    pub header_len: u8,
    pub flags: TcpFlags,
    pub window_size: Ne<u16>,
    pub checksum: Ne<u16>,
    pub urgent_pointer: Ne<u16>,
}

impl WriteableToPacket for TcpHeader {}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub(super) struct TcpFlags(u8);

impl TcpFlags {
    pub const FIN: Self = Self(1 << 0);
    pub const SYN: Self = Self(1 << 1);
    pub const RST: Self = Self(1 << 2);
    pub const PSH: Self = Self(1 << 3);
    pub const ACK: Self = Self(1 << 4);
    pub const URG: Self = Self(1 << 5);
    pub const ECE: Self = Self(1 << 6);
    pub const CWR: Self = Self(1 << 7);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn as_u8(self) -> u8 {
        self.0
    }

    pub fn contains(&self, flag: TcpFlags) -> bool {
        self.0 & flag.0 != 0
    }
}

impl BitOr<TcpFlags> for TcpFlags {
    type Output = TcpFlags;

    fn bitor(self, rhs: TcpFlags) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign<TcpFlags> for TcpFlags {
    fn bitor_assign(&mut self, rhs: TcpFlags) {
        self.0 |= rhs.0;
    }
}

impl fmt::Debug for TcpFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (value, name) in [
            (TcpFlags::FIN, "FIN"),
            (TcpFlags::SYN, "SYN"),
            (TcpFlags::RST, "RST"),
            (TcpFlags::PSH, "PSH"),
            (TcpFlags::ACK, "ACK"),
            (TcpFlags::URG, "URG"),
            (TcpFlags::ECE, "ECE"),
            (TcpFlags::CWR, "CWR"),
        ] {
            if self.0 & value.0 != 0 {
                if !first {
                    write!(f, "|")?;
                }

                write!(f, "{name}")?;
                first = false;
            }
        }

        Ok(())
    }
}
