#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Ipv4Addr(u32);

impl Ipv4Addr {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetMask(u32);

impl NetMask {
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        (ip.0 & self.0) == self.0
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Ipv4HeaderTooShort,
    NotIpv4,
    NotTcp,
    TcpHeaderTooShort,
    InvalidPacketLength,
    FragmentedPacket,
    InvalidIpv4Checksum,
    InvalidTcpChecksum,
}

pub struct Ipv4Inspector<'a> {
    buf: &'a [u8],
    ip_header_len: usize,
    header_len: usize,
    total_len: usize,
}

impl<'a> Ipv4Inspector<'a> {
    pub fn new_tcp_header(buf: &'a [u8]) -> Result<Self, Error> {
        if buf.len() < 20 {
            return Err(Error::Ipv4HeaderTooShort);
        }

        let version = buf[0] >> 4;
        if version != 4 {
            return Err(Error::NotIpv4);
        }

        let ihl = buf[0] & 0x0f;
        let ip_header_len = ihl as usize * 4;
        if ip_header_len < 20 || buf.len() < ip_header_len {
            return Err(Error::Ipv4HeaderTooShort);
        }

        if buf[9] != 6 {
            return Err(Error::NotTcp);
        }

        let tcp_offset = ip_header_len;
        if buf.len() < tcp_offset + 20 {
            return Err(Error::TcpHeaderTooShort);
        }

        let tcp_header_len = (buf[tcp_offset + 12] >> 4) as usize * 4;
        if tcp_header_len < 20 || buf.len() < tcp_offset + tcp_header_len {
            return Err(Error::TcpHeaderTooShort);
        }

        let total_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
        let header_len = ip_header_len + tcp_header_len;
        if total_len < header_len {
            return Err(Error::InvalidPacketLength);
        }

        Ok(Self {
            buf,
            ip_header_len,
            header_len,
            total_len,
        })
    }

    pub fn new_tcp_packet(buf: &'a [u8]) -> Result<Self, Error> {
        let inspector = Self::new_tcp_header(buf)?;
        if inspector.total_len > buf.len() {
            return Err(Error::InvalidPacketLength);
        }
        let fragment = u16::from_be_bytes([buf[6], buf[7]]);
        if fragment & 0x3fff != 0 {
            return Err(Error::FragmentedPacket);
        }
        if !checksum_valid(&buf[..inspector.ip_header_len], 0) {
            return Err(Error::InvalidIpv4Checksum);
        }

        let tcp_len = inspector.total_len - inspector.ip_header_len;
        let mut sum = 0;
        sum = checksum_add(sum, &buf[12..20]);
        sum += 6;
        sum += tcp_len as u32;
        if !checksum_valid(&buf[inspector.ip_header_len..inspector.total_len], sum) {
            return Err(Error::InvalidTcpChecksum);
        }
        Ok(inspector)
    }

    pub fn packet_len(&self) -> usize {
        self.total_len
    }

    pub fn transport_offset(&self) -> usize {
        self.ip_header_len
    }

    pub fn dst_ip(&self) -> Ipv4Addr {
        let value = u32::from_be_bytes(self.buf[16..20].try_into().unwrap());
        Ipv4Addr(value)
    }

    pub fn src_ip(&self) -> Ipv4Addr {
        let value = u32::from_be_bytes(self.buf[12..16].try_into().unwrap());
        Ipv4Addr(value)
    }

    pub fn dst_port(&self) -> u16 {
        let start = self.ip_header_len + 2;
        u16::from_be_bytes(self.buf[start..start + 2].try_into().unwrap())
    }

    pub fn src_port(&self) -> u16 {
        let start = self.ip_header_len;
        u16::from_be_bytes(self.buf[start..start + 2].try_into().unwrap())
    }

    pub fn seq(&self) -> u32 {
        let start = self.ip_header_len + 4;
        u32::from_be_bytes(self.buf[start..start + 4].try_into().unwrap())
    }

    pub fn ack(&self) -> u32 {
        let start = self.ip_header_len + 8;
        u32::from_be_bytes(self.buf[start..start + 4].try_into().unwrap())
    }

    pub fn flags(&self) -> u8 {
        self.buf[self.ip_header_len + 13]
    }

    pub fn window_size(&self) -> u16 {
        let start = self.ip_header_len + 14;
        u16::from_be_bytes(self.buf[start..start + 2].try_into().unwrap())
    }

    pub fn payload_offset(&self) -> usize {
        self.header_len
    }

    pub fn payload_len(&self) -> usize {
        self.total_len - self.header_len
    }
}

fn checksum_valid(bytes: &[u8], initial: u32) -> bool {
    let mut sum = checksum_add(initial, bytes);
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    sum as u16 == 0xffff
}

fn checksum_add(mut sum: u32, bytes: &[u8]) -> u32 {
    let mut chunks = bytes.chunks_exact(2);
    for chunk in &mut chunks {
        sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
    }
    if let Some(byte) = chunks.remainder().first() {
        sum += (*byte as u32) << 8;
    }
    sum
}
