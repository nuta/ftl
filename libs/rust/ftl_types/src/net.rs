#[derive(Clone, Copy)]
#[repr(C)]
pub struct NetRxInfo {
    pub remote_ip: u32,
    pub local_ip: u32,
    pub remote_port: u16,
    pub local_port: u16,
    pub seq: u32,
    pub ack: u32,
    pub payload_len: u16,
    pub window_size: u16,
    pub flags: u8,
    pub reserved: [u8; 3],
}

impl NetRxInfo {
    pub const fn empty() -> Self {
        Self {
            remote_ip: 0,
            local_ip: 0,
            remote_port: 0,
            local_port: 0,
            seq: 0,
            ack: 0,
            payload_len: 0,
            window_size: 0,
            flags: 0,
            reserved: [0; 3],
        }
    }
}

pub const NET_IPV4: usize = 1 << 0;
pub const NET_TCP: usize = 1 << 1;
pub const NET_LISTEN: usize = 1 << 2;
