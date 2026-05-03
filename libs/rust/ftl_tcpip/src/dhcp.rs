use crate::endian::Ne;
use crate::ethernet::EthernetHeader;
use crate::ethernet::MacAddr;
use crate::ip::Ipv4Addr;
use crate::ip::NetMask;
use crate::ip::ipv4::Ipv4Header;
use crate::packet::Packet;
use crate::packet::WriteableToPacket;
use crate::packet::{self};
use crate::transport::Port;
use crate::udp::UdpHeader;

const MAGIC_COOKIE: u32 = 0x63825363;
const BOOT_REQUEST: u8 = 1;
const FLAG_BROADCAST: u16 = 0x8000;

/// DHCP message length must be at least 300 bytes as per RFC 1542:
///
/// > The IP Total Length and UDP Length must be large enough to
/// > contain the minimal BOOTP header of 300 octets (in the UDP
/// > data field) specified in [1].
/// >
/// > https://datatracker.ietf.org/doc/html/rfc1542
const MIN_PAYLOAD_LEN: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub(crate) struct OptionType(u8);

impl OptionType {
    const PAD: Self = Self(0);
    const SUBNET_MASK: Self = Self(1);
    const ROUTER: Self = Self(3);
    const REQUESTED_IP_ADDRESS: Self = Self(50);
    const DHCP_MESSAGE_TYPE: Self = Self(53);
    const SERVER_IDENTIFIER: Self = Self(54);
    const PARAMETER_REQUEST_LIST: Self = Self(55);
    const END: Self = Self(0xff);
}

impl WriteableToPacket for OptionType {}

const REQUESTED_PARAMETERS: &[u8] = &[OptionType::SUBNET_MASK.0, OptionType::ROUTER.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub(crate) struct Opcode(u8);

impl Opcode {
    const DISCOVER: Self = Self(1);
    const OFFER: Self = Self(2);
    const REQUEST: Self = Self(3);
    const ACK: Self = Self(5);
}

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
    type_: OptionType,
    length: u8,
    opcode: Opcode,
}

impl WriteableToPacket for DhcpMessageTypeOption {}

pub(crate) struct Tx {
    pub local_ip: Ipv4Addr,
    pub remote_ip: Ipv4Addr,
    pub remote_port: Port,
    pub pkt: Packet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Lease {
    pub addr: Ipv4Addr,
    pub subnet_mask: NetMask,
    pub router: Option<Ipv4Addr>,
}

#[derive(Debug)]
pub(crate) enum TxError {
    PacketAlloc(#[expect(unused)] packet::AllocError),
    PacketWrite(#[expect(unused)] packet::ReserveError),
}

#[derive(Debug)]
pub(crate) enum RxError {
    PacketRead(#[expect(unused)] packet::ReserveError),
    BadMagicCookie(#[expect(unused)] u32),
    ReceivedBeforeDiscover(#[expect(unused)] Opcode),
    ReceivedBeforeRequest(#[expect(unused)] Opcode),
    ExpectedOffer(#[expect(unused)] Opcode),
    ExpectedAck(#[expect(unused)] Opcode),
    ReceivedAfterAck(#[expect(unused)] Opcode),
    InvalidOptionLength {
        #[expect(unused)]
        option_type: OptionType,
        #[expect(unused)]
        len: u8,
    },
    MissingDhcpMessageTypeOption,
    MissingSubnetMaskOption,
}
enum State {
    BeforeDiscover,
    SentDiscover,
    BeforeRequest {
        request_ip: Ipv4Addr,
        server_identifier: Option<Ipv4Addr>,
    },
    SentRequest,
    Connected,
}

pub(crate) struct DhcpClient {
    state: spin::Mutex<State>,
    mac: MacAddr,
}

impl DhcpClient {
    pub fn new(mac: MacAddr) -> Self {
        Self {
            state: spin::Mutex::new(State::BeforeDiscover),
            mac,
        }
    }

    pub fn handle_rx(&mut self, pkt: &mut Packet) -> Result<Option<Lease>, RxError> {
        let header = pkt.read::<DhcpHeader>().map_err(RxError::PacketRead)?;
        let magic = header.magic.into();
        if magic != MAGIC_COOKIE {
            return Err(RxError::BadMagicCookie(magic));
        }

        let your_ip = Ipv4Addr::from(header.your_ip);
        let mut parser = OptionParser::new();
        parser.parse(pkt)?;
        let opcode = parser
            .message_type
            .ok_or(RxError::MissingDhcpMessageTypeOption)?;

        let mut state = self.state.lock();
        match *state {
            State::BeforeDiscover => {
                // We haven't sent a discover packet yet. We don't know what to
                // do with this packet.
                Err(RxError::ReceivedBeforeDiscover(opcode))
            }
            State::SentDiscover => {
                if opcode != Opcode::OFFER {
                    return Err(RxError::ExpectedOffer(opcode));
                }

                trace!("DHCP: received OFFER: your_ip={:?}", your_ip);
                *state = State::BeforeRequest {
                    request_ip: your_ip,
                    server_identifier: parser.server_identifier,
                };

                Ok(None)
            }
            State::BeforeRequest { .. } => Err(RxError::ReceivedBeforeRequest(opcode)),
            State::SentRequest => {
                if opcode != Opcode::ACK {
                    return Err(RxError::ExpectedAck(opcode));
                }

                let subnet_mask = parser.subnet_mask.ok_or(RxError::MissingSubnetMaskOption)?;
                let lease = Lease {
                    addr: your_ip,
                    subnet_mask,
                    router: parser.router,
                };

                trace!("DHCP: received ACK: your_ip={:?}", your_ip);
                *state = State::Connected;
                Ok(Some(lease))
            }
            State::Connected { .. } => Err(RxError::ReceivedAfterAck(opcode)),
        }
    }

    pub fn poll_tx(&mut self) -> Result<Option<Tx>, TxError> {
        let mut client_mac = [0; 16];
        client_mac[..6].copy_from_slice(self.mac.as_bytes());

        let mut state = self.state.lock();
        match *state {
            State::BeforeDiscover => {
                trace!("DHCP: sending DISCOVER");
                let header = DhcpHeader {
                    opcode: BOOT_REQUEST,
                    hardware_type: 1,
                    hwaddr_length: 6,
                    hops: 0,
                    transaction_id: 0x12345678.into(),
                    seconds: 0.into(),
                    flags: FLAG_BROADCAST.into(),
                    client_ip: 0.into(),
                    your_ip: 0.into(),
                    server_ip: 0.into(),
                    gateway_ip: 0.into(),
                    client_mac,
                    server_name: [0; 64],
                    file: [0; 128],
                    magic: MAGIC_COOKIE.into(),
                };

                let len = dhcp_message_len(
                    size_of::<DhcpMessageTypeOption>() + 2 + REQUESTED_PARAMETERS.len() + 1,
                );
                let head_room =
                    size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<UdpHeader>();
                let mut pkt = Packet::new(len, head_room).map_err(TxError::PacketAlloc)?;

                pkt.write_back(header).map_err(TxError::PacketWrite)?;
                let mut options_writer = OptionWriter::new(pkt, Opcode::DISCOVER)?;
                options_writer
                    .write_parameter_request_list(REQUESTED_PARAMETERS)
                    .map_err(TxError::PacketWrite)?;
                let pkt = options_writer.finish().map_err(TxError::PacketWrite)?;

                *state = State::SentDiscover;
                Ok(Some(Tx {
                    local_ip: Ipv4Addr::UNSPECIFIED,
                    remote_ip: Ipv4Addr::BROADCAST,
                    remote_port: Port::new(67),
                    pkt,
                }))
            }
            State::SentDiscover => Ok(None),
            State::BeforeRequest {
                request_ip,
                server_identifier,
            } => {
                trace!(
                    "DHCP: sending REQUEST: request_ip={:?}, server_identifier={:?}",
                    request_ip, server_identifier
                );
                let header = DhcpHeader {
                    opcode: BOOT_REQUEST,
                    hardware_type: 1,
                    hwaddr_length: 6,
                    hops: 0,
                    transaction_id: 0x12345678.into(),
                    seconds: 0.into(),
                    flags: FLAG_BROADCAST.into(),
                    client_ip: 0.into(),
                    your_ip: 0.into(),
                    server_ip: 0.into(),
                    gateway_ip: 0.into(),
                    client_mac,
                    server_name: [0; 64],
                    file: [0; 128],
                    magic: MAGIC_COOKIE.into(),
                };

                let server_identifier_len = if server_identifier.is_some() { 6 } else { 0 };
                let len = dhcp_message_len(
                    size_of::<DhcpMessageTypeOption>()
                        + 6
                        + server_identifier_len
                        + 2
                        + REQUESTED_PARAMETERS.len()
                        + 1,
                );
                let head_room =
                    size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<UdpHeader>();
                let mut pkt = Packet::new(len, head_room).map_err(TxError::PacketAlloc)?;
                pkt.write_back(header).map_err(TxError::PacketWrite)?;

                let mut options_writer = OptionWriter::new(pkt, Opcode::REQUEST)?;
                options_writer
                    .write_ipv4_option(OptionType::REQUESTED_IP_ADDRESS, request_ip)
                    .map_err(TxError::PacketWrite)?;
                if let Some(server_identifier) = server_identifier {
                    options_writer
                        .write_ipv4_option(OptionType::SERVER_IDENTIFIER, server_identifier)
                        .map_err(TxError::PacketWrite)?;
                }
                options_writer
                    .write_parameter_request_list(REQUESTED_PARAMETERS)
                    .map_err(TxError::PacketWrite)?;
                let pkt = options_writer.finish().map_err(TxError::PacketWrite)?;

                *state = State::SentRequest;
                Ok(Some(Tx {
                    local_ip: Ipv4Addr::UNSPECIFIED,
                    remote_ip: Ipv4Addr::BROADCAST,
                    remote_port: Port::new(67),
                    pkt,
                }))
            }
            State::SentRequest => Ok(None),
            State::Connected { .. } => Ok(None),
        }
    }
}

fn dhcp_message_len(options_len: usize) -> usize {
    (size_of::<DhcpHeader>() + options_len).max(MIN_PAYLOAD_LEN)
}

struct OptionParser {
    message_type: Option<Opcode>,
    subnet_mask: Option<NetMask>,
    router: Option<Ipv4Addr>,
    server_identifier: Option<Ipv4Addr>,
}

impl OptionParser {
    fn new() -> Self {
        Self {
            message_type: None,
            subnet_mask: None,
            router: None,
            server_identifier: None,
        }
    }

    fn parse(&mut self, pkt: &mut Packet) -> Result<(), RxError> {
        loop {
            let type_raw = pkt.read::<u8>().map_err(RxError::PacketRead)?;
            let option_type = OptionType(*type_raw);
            if option_type == OptionType::PAD {
                continue;
            }
            if option_type == OptionType::END {
                break;
            }

            let len = *pkt.read::<u8>().map_err(RxError::PacketRead)?;
            match option_type {
                OptionType::DHCP_MESSAGE_TYPE => {
                    if len != 1 {
                        return Err(RxError::InvalidOptionLength { option_type, len });
                    }

                    let raw = *pkt.read::<u8>().map_err(RxError::PacketRead)?;
                    self.message_type = Some(Opcode(raw));
                }
                OptionType::SUBNET_MASK => {
                    if len != 4 {
                        return Err(RxError::InvalidOptionLength { option_type, len });
                    }

                    let raw = pkt.read::<[u8; 4]>().map_err(RxError::PacketRead)?;
                    let netmask = NetMask::new(raw[0], raw[1], raw[2], raw[3]);
                    self.subnet_mask = Some(netmask);
                }
                OptionType::ROUTER => {
                    // Router option may have multiple addresses, but each must be 4 bytes long.
                    if len < 4 || len % 4 != 0 {
                        return Err(RxError::InvalidOptionLength { option_type, len });
                    }

                    let raw = pkt.read::<[u8; 4]>().map_err(RxError::PacketRead)?;
                    let router = Ipv4Addr::new(raw[0], raw[1], raw[2], raw[3]);
                    self.router = Some(router);
                    pkt.discard(len as usize - 4).map_err(RxError::PacketRead)?;
                }
                OptionType::SERVER_IDENTIFIER => {
                    if len != 4 {
                        return Err(RxError::InvalidOptionLength { option_type, len });
                    }

                    let raw = pkt.read::<[u8; 4]>().map_err(RxError::PacketRead)?;
                    let server_identifier = Ipv4Addr::new(raw[0], raw[1], raw[2], raw[3]);
                    self.server_identifier = Some(server_identifier);
                }
                _ => {
                    pkt.discard(len as usize).map_err(RxError::PacketRead)?;
                }
            }
        }

        Ok(())
    }
}

struct OptionWriter {
    pkt: Packet,
}

impl OptionWriter {
    fn new(mut pkt: Packet, message_type: Opcode) -> Result<Self, TxError> {
        pkt.write_back(DhcpMessageTypeOption {
            type_: OptionType::DHCP_MESSAGE_TYPE,
            length: 1,
            opcode: message_type,
        })
        .map_err(TxError::PacketWrite)?;

        Ok(Self { pkt })
    }

    fn write_option(
        &mut self,
        option_type: OptionType,
        data: &[u8],
    ) -> Result<(), packet::ReserveError> {
        self.pkt.write_back(option_type)?;
        self.pkt.write_back_bytes(&[data.len() as u8])?;
        self.pkt.write_back_bytes(data)?;
        Ok(())
    }

    fn write_parameter_request_list(
        &mut self,
        parameters: &[u8],
    ) -> Result<(), packet::ReserveError> {
        self.write_option(OptionType::PARAMETER_REQUEST_LIST, parameters)?;
        Ok(())
    }

    fn write_ipv4_option(
        &mut self,
        option_type: OptionType,
        addr: Ipv4Addr,
    ) -> Result<(), packet::ReserveError> {
        self.write_option(option_type, &addr.as_u32().to_be_bytes())
    }

    fn write_end(&mut self) -> Result<(), packet::ReserveError> {
        self.pkt.write_back(OptionType::END)
    }

    fn finish(mut self) -> Result<Packet, packet::ReserveError> {
        self.write_end()?;
        while self.pkt.len() < MIN_PAYLOAD_LEN {
            self.pkt.write_back(OptionType::PAD)?;
        }
        Ok(self.pkt)
    }
}
