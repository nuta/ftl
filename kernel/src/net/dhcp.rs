use ftl_types::error::ErrorCode;
use ftl_utils::spinlock::SpinLock;

use super::device::Device;
use super::packet::dhcp::DHCP_ACK;
use super::packet::dhcp::DHCP_BROADCAST;
use super::packet::dhcp::DHCP_CLIENT_PORT;
use super::packet::dhcp::DHCP_DISCOVER;
use super::packet::dhcp::DHCP_ETHERNET_ADDR_LEN;
use super::packet::dhcp::DHCP_HW_ETHERNET;
use super::packet::dhcp::DHCP_OFFER;
use super::packet::dhcp::DHCP_OP_REQUEST;
use super::packet::dhcp::DHCP_REQUEST;
use super::packet::dhcp::DHCP_SERVER_PORT;
use super::packet::dhcp::DhcpInspector;
use super::packet::dhcp::DhcpRewriter;
use super::packet::dhcp::OPTION_MESSAGE_TYPE;
use super::packet::dhcp::OPTION_PARAM_REQUEST_LIST;
use super::packet::dhcp::OPTION_REQUESTED_IP;
use super::packet::dhcp::OPTION_ROUTER;
use super::packet::dhcp::OPTION_SERVER_ID;
use super::packet::dhcp::OPTION_SUBNET_MASK;
use super::packet::ipv4::Ipv4Addr;
use super::packet::ipv4::Ipv4Inspector;
use super::packet::ipv4::NetMask;
use super::packet::udp::UdpInspector;
use super::udp;
use crate::shared_ref::SharedRef;

const DHCP_PACKET_LEN: usize = 300;
const TX_ID: u32 = 0x1234_5678;

pub struct DhcpConfig {
    pub address: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub netmask: NetMask,
}

/// The state of a DHCP client.
enum State {
    Discovering,
    Requesting(Request),
    Bound,
}

/// A request to the DHCP server.
#[derive(Clone, Copy)]
struct Request {
    address: Ipv4Addr,
    server: Ipv4Addr,
}

/// A DHCP client.
struct Client {
    mac: [u8; 6],
    state: State,
}

impl Client {
    fn new(mac: [u8; 6]) -> Self {
        Self {
            mac,
            state: State::Discovering,
        }
    }

    fn handle_rx(
        &mut self,
        device: &SharedRef<Device>,
        ipv4: &Ipv4Inspector<'_>,
        udp: &UdpInspector<'_>,
    ) -> Option<DhcpConfig> {
        let packet = match DhcpInspector::new(udp.payload()) {
            Ok(packet) => packet,
            Err(error) => {
                trace!("failed to inspect DHCP packet: {:?}", error);
                return None;
            }
        };

        let message_type = match packet.message_type() {
            Ok(Some(message_type)) => message_type,
            Ok(None) => return None,
            Err(error) => {
                trace!("failed to inspect DHCP options: {:?}", error);
                return None;
            }
        };

        if packet.tx_id() != TX_ID {
            return None;
        }

        if packet.client_hwaddr() != self.mac {
            return None;
        }

        if message_type == DHCP_ACK {
            return self.handle_ack(&packet);
        }

        if message_type != DHCP_OFFER {
            return None;
        }

        let request = self.handle_offer(&packet, ipv4.src_ip())?;
        if let Err(error) = self.send_request(device, request) {
            warn!("failed to send DHCP request: {:?}", error);
        }
        None
    }

    fn handle_offer(&mut self, pkt: &DhcpInspector<'_>, source: Ipv4Addr) -> Option<Request> {
        if !matches!(self.state, State::Discovering) {
            return None;
        }

        let server = match pkt.ipv4_option(OPTION_SERVER_ID) {
            Ok(Some(server)) => server,
            Ok(None) => source,
            Err(_) => return None,
        };

        let request = Request {
            address: pkt.your_ip(),
            server,
        };

        self.state = State::Requesting(request);
        Some(request)
    }

    fn handle_ack(&mut self, pkt: &DhcpInspector<'_>) -> Option<DhcpConfig> {
        let State::Requesting(request) = self.state else {
            return None;
        };

        let assigned_address = pkt.your_ip();
        if assigned_address != Ipv4Addr::new(0) && assigned_address != request.address {
            return None;
        }

        match pkt.ipv4_option(OPTION_SERVER_ID) {
            Ok(Some(actual)) if actual != request.server => return None,
            Err(_) => return None,
            _ => {}
        }

        let gateway = match pkt.ipv4_option(OPTION_ROUTER) {
            Ok(Some(gateway)) => gateway,
            _ => return None,
        };

        let netmask = match pkt.ipv4_option(OPTION_SUBNET_MASK) {
            Ok(Some(netmask)) => NetMask::new(netmask.as_u32()),
            Ok(None) => NetMask::new(0),
            Err(_) => return None,
        };

        self.state = State::Bound;
        Some(DhcpConfig {
            address: request.address,
            gateway,
            netmask,
        })
    }

    fn send_discover(&self, device: &SharedRef<Device>) -> Result<(), ErrorCode> {
        self.do_send(device, DHCP_DISCOVER, |packet| {
            packet.write_option(
                OPTION_PARAM_REQUEST_LIST,
                &[OPTION_SUBNET_MASK, OPTION_ROUTER, OPTION_SERVER_ID],
            )?;
            Ok(())
        })
    }

    fn send_request(&self, device: &SharedRef<Device>, req: Request) -> Result<(), ErrorCode> {
        self.do_send(device, DHCP_REQUEST, |packet| {
            packet.write_option(OPTION_REQUESTED_IP, &req.address.as_u32().to_be_bytes())?;
            packet.write_option(OPTION_SERVER_ID, &req.server.as_u32().to_be_bytes())?;
            packet.write_option(
                OPTION_PARAM_REQUEST_LIST,
                &[OPTION_SUBNET_MASK, OPTION_ROUTER],
            )?;
            Ok(())
        })
    }

    fn do_send(
        &self,
        device: &SharedRef<Device>,
        message_type: u8,
        option_writer: impl FnOnce(&mut DhcpRewriter<'_>) -> Result<(), super::packet::dhcp::Error>,
    ) -> Result<(), ErrorCode> {
        let mut bytes = [0u8; DHCP_PACKET_LEN];
        let mut packet = DhcpRewriter::new(&mut bytes).unwrap();
        packet.set_op(DHCP_OP_REQUEST);
        packet.set_hw_type(DHCP_HW_ETHERNET);
        packet.set_hwaddr_len(DHCP_ETHERNET_ADDR_LEN);
        packet.set_tx_id(TX_ID);
        packet.set_flags(DHCP_BROADCAST);
        packet.set_client_hwaddr(self.mac);

        packet
            .write_option(OPTION_MESSAGE_TYPE, &[message_type])
            .unwrap();

        if let Err(error) = option_writer(&mut packet) {
            // This should not fail, but option_writer may add too many
            // options.
            warn!("failed to write DHCP options: {:?}", error);
            return Err(ErrorCode::INVALID_ARG);
        }

        if let Err(error) = packet.finish() {
            // This should not fail, but option_writer may add too many
            // options.
            warn!("failed to finish DHCP packet: {:?}", error);
            return Err(ErrorCode::INVALID_ARG);
        }

        udp::send_broadcast(device, DHCP_CLIENT_PORT, DHCP_SERVER_PORT, &bytes)
    }
}

static CLIENT: SpinLock<Option<Client>> = SpinLock::new(None);

pub fn start(device: &SharedRef<Device>) {
    let mac = *device.driver().mac_address();
    let mut client = CLIENT.lock();
    *client = Some(Client::new(mac));
    if let Err(error) = client.as_ref().unwrap().send_discover(device) {
        warn!("failed to send DHCP discover: {:?}", error);
    }
}

pub fn handle_rx(
    device: &SharedRef<Device>,
    ipv4: &Ipv4Inspector<'_>,
    udp: &UdpInspector<'_>,
) -> Option<DhcpConfig> {
    CLIENT.lock().as_mut()?.handle_rx(device, ipv4, udp)
}
