pub(super) struct Checksum {
    sum: u32,
    pending_byte: Option<u8>,
}

impl Checksum {
    pub const fn new() -> Self {
        Self {
            sum: 0,
            pending_byte: None,
        }
    }

    pub fn add(mut self, mut bytes: &[u8]) -> Self {
        if let Some(high) = self.pending_byte.take() {
            if let Some((&low, rest)) = bytes.split_first() {
                self.sum += u16::from_be_bytes([high, low]) as u32;
                bytes = rest;
            } else {
                self.pending_byte = Some(high);
                return self;
            }
        }

        let mut chunks = bytes.chunks_exact(2);
        for chunk in &mut chunks {
            self.sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
        }
        self.pending_byte = chunks.remainder().first().copied();
        self
    }

    pub fn add_u16(self, value: u16) -> Self {
        self.add(&value.to_be_bytes())
    }

    pub fn value(self) -> u16 {
        !self.fold()
    }

    pub fn is_valid(self) -> bool {
        self.fold() == 0xffff
    }

    fn fold(mut self) -> u16 {
        if let Some(high) = self.pending_byte {
            self.sum += (high as u32) << 8;
        }
        while self.sum >> 16 != 0 {
            self.sum = (self.sum & 0xffff) + (self.sum >> 16);
        }
        self.sum as u16
    }
}
