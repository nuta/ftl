use crate::endian::Ne;
use crate::ethernet::EthernetHeader;
use crate::ethernet::MacAddr;
use crate::ip::Ipv4Addr;
use crate::ip::ipv4::Ipv4Header;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::packet::{self};
use crate::transport::Port;
use crate::udp::UdpHeader;

const MAGIC_COOKIE: u32 = 0x63825363;

#[repr(C, packed)]
struct DhcpHeader {
    opcode: u8,
    hardware_type: u8,
    hwaddr_length: u8,
    hops: u8,
    transaction_id: Ne<u32>,
    seconds: Ne<u16>,
    flags: Ne<u16>,
    client_ip: Ne<u32>,
    your_ip: Ne<u32>,
    server_ip: Ne<u32>,
    gateway_ip: Ne<u32>,
    client_mac: [u8; 16],
    server_name: [u8; 64],
    file: [u8; 128],
    magic: Ne<u32>,
}

impl WriteableToPacket for DhcpHeader {}

#[repr(C, packed)]
struct DhcpMessageTypeOption {
    type_: u8,
    length: u8,
    value: u8,
}

impl WriteableToPacket for DhcpMessageTypeOption {}

pub(crate) struct Tx {
    pub local_ip: Ipv4Addr,
    pub remote_ip: Ipv4Addr,
    pub remote_port: Port,
    pub pkt: Packet,
}

#[derive(Debug)]
pub(crate) enum TxError {
    PacketAlloc(packet::AllocError),
    PacketWrite(packet::ReserveError),
}

#[derive(Debug)]
pub(crate) enum RxError {
    RxInInitState,
}

enum State {
    Init,
    SentDiscover,
}

pub(crate) struct DhcpClient {
    state: spin::Mutex<State>,
    mac: MacAddr,
}

impl DhcpClient {
    pub fn new(mac: MacAddr) -> Self {
        Self {
            state: spin::Mutex::new(State::Init),
            mac,
        }
    }

    pub fn poll_tx(&mut self) -> Result<Option<Tx>, TxError> {
        let mut client_mac = [0; 16];
        client_mac[..6].copy_from_slice(self.mac.as_bytes());

        let state = self.state.lock();
        match *state {
            State::Init => {
                let header = DhcpHeader {
                    opcode: 1, // DHCPDISCOVER
                    hardware_type: 1,
                    hwaddr_length: 6,
                    hops: 0,
                    transaction_id: 0x12345678.into(),
                    seconds: 0.into(),
                    flags: 0.into(),
                    client_ip: 0.into(),
                    your_ip: 0.into(),
                    server_ip: 0.into(),
                    gateway_ip: 0.into(),
                    client_mac,
                    server_name: [0; 64],
                    file: [0; 128],
                    magic: MAGIC_COOKIE.into(),
                };

                let type_option = DhcpMessageTypeOption {
                    type_: 53, // DHCP Message Type
                    length: 1, // in bytes
                    value: 1,  // DHCPDISCOVER
                };

                let len = size_of::<DhcpHeader>() + size_of::<DhcpMessageTypeOption>() + 1;
                let head_room =
                    size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<UdpHeader>();
                let mut pkt = Packet::new(len, head_room).map_err(TxError::PacketAlloc)?;
                pkt.write_back(header).map_err(TxError::PacketWrite)?;
                pkt.write_back(type_option).map_err(TxError::PacketWrite)?;
                pkt.write_back_bytes(&[0xff])
                    .map_err(TxError::PacketWrite)?; // End

                Ok(Some(Tx {
                    local_ip: Ipv4Addr::UNSPECIFIED,
                    remote_ip: Ipv4Addr::BROADCAST,
                    remote_port: Port::new(67),
                    pkt,
                }))
            }
            State::SentDiscover => Ok(None),
        }
    }

    pub fn handle_rx(&mut self, data: &[u8]) -> Result<(), RxError> {
        let state = self.state.lock();
        match *state {
            State::Init => {
                // We haven't sent a discover packet yet. We don't know what to
                // do with this packet.
                Err(RxError::RxInInitState)
            }
            State::SentDiscover => {
                trace!("DHCP: received: {:x?}", data);
                Ok(())
            }
        }
    }
}
