/// Checksum calculation helper for IP and TCP.
pub struct Checksum(u64);

impl Checksum {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn supply_bytes(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(2);
        for chunk in &mut chunks {
            self.add_word(u16::from_be_bytes([chunk[0], chunk[1]]));
        }

        if let Some(&byte) = chunks.remainder().first() {
            self.add_word(u16::from_be_bytes([byte, 0]));
        }
    }

    pub fn supply_u16(&mut self, value: u16) {
        self.add_word(value);
    }

    pub fn supply_u32(&mut self, value: u32) {
        self.add_word((value >> 16) as u16);
        self.add_word(value as u16);
    }

    pub fn finish(&self) -> u16 {
        let mut sum = self.0;

        while sum >> 16 != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }

        !(sum as u16)
    }

    fn add_word(&mut self, word: u16) {
        self.0 = self.0.wrapping_add(word as u64);
    }
}
