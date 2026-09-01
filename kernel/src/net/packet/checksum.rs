use crate::net::packet::ipv4::Ipv4Addr;

pub struct Checksum {
    sum: u32,
}

impl Checksum {
    pub const fn new() -> Self {
        Self { sum: 0 }
    }

    pub fn add_u16(&mut self, value: u16) {
        self.sum += u32::from(value);
    }

    pub fn add_ipv4(&mut self, ip: Ipv4Addr) {
        let bytes = ip.as_u32().to_be_bytes();
        self.add_u16(u16::from_be_bytes([bytes[0], bytes[1]]));
        self.add_u16(u16::from_be_bytes([bytes[2], bytes[3]]));
    }

    pub fn add_bytes(&mut self, bytes: &[u8]) {
        // Sum 16-bit words.
        let mut chunks = bytes.chunks_exact(2);
        for chunk in &mut chunks {
            self.sum += u32::from(u16::from_be_bytes([chunk[0], chunk[1]]));
        }

        // Sum the final byte if bytes are not 16-bit aligned.
        if bytes.len() % 2 != 0 {
            self.sum += u32::from(bytes[bytes.len() - 1]) << 8;
        }
    }

    pub fn finish(mut self) -> u16 {
        // Fold the sum into a 16-bit value.
        while self.sum >> 16 != 0 {
            self.sum = (self.sum & 0xffff) + (self.sum >> 16);
        }

        // Return the one's complement of the sum.
        !(self.sum as u16)
    }
}
