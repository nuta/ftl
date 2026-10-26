use ftl_driver::env::Env;
use ftl_driver::trace;
use ftl_driver::warn;
use ftl_types::error::ErrorCode;
use ftl_types::time::Duration;
use ftl_types::time::MonoTime;

use crate::device::Device;
use crate::packet::dhcp::DHCP_ACK;
use crate::packet::dhcp::DHCP_BROADCAST;
use crate::packet::dhcp::DHCP_CLIENT_PORT;
use crate::packet::dhcp::DHCP_DISCOVER;
use crate::packet::dhcp::DHCP_ETHERNET_ADDR_LEN;
use crate::packet::dhcp::DHCP_HW_ETHERNET;
use crate::packet::dhcp::DHCP_OFFER;
use crate::packet::dhcp::DHCP_OP_REQUEST;
use crate::packet::dhcp::DHCP_REQUEST;
use crate::packet::dhcp::DHCP_SERVER_PORT;
use crate::packet::dhcp::DhcpInspector;
use crate::packet::dhcp::DhcpRewriter;
use crate::packet::dhcp::Error;
use crate::packet::dhcp::OPTION_MESSAGE_TYPE;
use crate::packet::dhcp::OPTION_PARAM_REQUEST_LIST;
use crate::packet::dhcp::OPTION_REQUESTED_IP;
use crate::packet::dhcp::OPTION_ROUTER;
use crate::packet::dhcp::OPTION_SERVER_ID;
use crate::packet::dhcp::OPTION_SUBNET_MASK;
use crate::packet::ipv4::Ipv4Addr;
use crate::packet::ipv4::Ipv4Inspector;
use crate::packet::ipv4::NetMask;
use crate::packet::udp::UdpInspector;
use crate::udp;

const DHCP_PACKET_LEN: usize = 300;
const TX_ID: u32 = 0x1234_5678;
const MAX_RETRIES: u32 = 10;

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
    Failed,
}

/// A request to the DHCP server.
#[derive(Clone, Copy)]
struct Request {
    address: Ipv4Addr,
    server: Ipv4Addr,
}

/// A DHCP client.
pub(crate) struct Client {
    mac: [u8; 6],
    state: State,
    retry_at: Option<MonoTime>,
    retries: u32,
}

impl Client {
    pub(crate) fn new(mac: [u8; 6]) -> Self {
        Self {
            mac,
            state: State::Discovering,
            retry_at: None,
            retries: 0,
        }
    }

    pub(crate) fn retransmit<'a>(&mut self, env: &'a dyn Env, device: &Device<'a>, now: MonoTime) {
        let Some(deadline) = self.retry_at else {
            // Nothing to retry.
            return;
        };

        if now.duration_since(deadline).is_none() {
            // Too early to retry.
            return;
        }

        if self.retries == MAX_RETRIES {
            let state_str = match self.state {
                State::Discovering => "discovery",
                State::Requesting(_) => "request",
                State::Bound | State::Failed => return,
            };

            warn!(env, "DHCP {state_str} timed out");
            self.set_state(State::Failed);
            return;
        }

        self.retries += 1;
        let result = match self.state {
            State::Requesting(request) => self.send_request(env, device, request, now),
            _ => self.send_discover(env, device, now),
        };

        if let Err(error) = result {
            trace!(env, "failed to retransmit DHCP packet: {:?}", error);
        }
    }

    /// Transitions to a new state.
    fn set_state(&mut self, state: State) {
        self.state = state;

        // Reset the retry counter.
        self.retries = 0;
        self.retry_at = None;
    }

    pub(crate) fn handle_rx<'a>(
        &mut self,
        env: &'a dyn Env,
        device: &Device<'a>,
        ipv4: &Ipv4Inspector<'_>,
        udp: &UdpInspector<'_>,
        now: MonoTime,
    ) -> Option<DhcpConfig> {
        let packet = match DhcpInspector::new(udp.payload()) {
            Ok(packet) => packet,
            Err(error) => {
                ftl_driver::trace!(env, "failed to inspect DHCP packet: {:?}", error);
                return None;
            }
        };

        let message_type = match packet.message_type() {
            Ok(Some(message_type)) => message_type,
            Ok(None) => return None,
            Err(error) => {
                ftl_driver::trace!(env, "failed to inspect DHCP options: {:?}", error);
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
        if let Err(error) = self.send_request(env, device, request, now) {
            ftl_driver::warn!(env, "failed to send DHCP request: {:?}", error);
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

        self.set_state(State::Requesting(request));
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
            Ok(Some(netmask)) if netmask.as_u32() != 0 => NetMask::new(netmask.as_u32()),
            _ => return None,
        };

        self.set_state(State::Bound);
        Some(DhcpConfig {
            address: request.address,
            gateway,
            netmask,
        })
    }

    pub(crate) fn send_discover<'a>(
        &mut self,
        env: &'a dyn Env,
        device: &Device<'a>,
        now: MonoTime,
    ) -> Result<(), ErrorCode> {
        self.do_send(env, device, now, DHCP_DISCOVER, |packet| {
            packet.write_option(
                OPTION_PARAM_REQUEST_LIST,
                &[OPTION_SUBNET_MASK, OPTION_ROUTER, OPTION_SERVER_ID],
            )?;
            Ok(())
        })
    }

    fn send_request<'a>(
        &mut self,
        env: &'a dyn Env,
        device: &Device<'a>,
        req: Request,
        now: MonoTime,
    ) -> Result<(), ErrorCode> {
        self.do_send(env, device, now, DHCP_REQUEST, |packet| {
            packet.write_option(OPTION_REQUESTED_IP, &req.address.as_u32().to_be_bytes())?;
            packet.write_option(OPTION_SERVER_ID, &req.server.as_u32().to_be_bytes())?;
            packet.write_option(
                OPTION_PARAM_REQUEST_LIST,
                &[OPTION_SUBNET_MASK, OPTION_ROUTER],
            )?;
            Ok(())
        })
    }

    fn do_send<'a>(
        &mut self,
        env: &'a dyn Env,
        device: &Device<'a>,
        now: MonoTime,
        message_type: u8,
        option_writer: impl FnOnce(&mut DhcpRewriter<'_>) -> Result<(), Error>,
    ) -> Result<(), ErrorCode> {
        let retry_after = Duration::from_millis((100 << self.retries).min(1000));
        self.retry_at = Some(now + retry_after);

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
            ftl_driver::warn!(env, "failed to write DHCP options: {:?}", error);
            return Err(ErrorCode::InvalidArg);
        }

        if let Err(error) = packet.finish() {
            // This should not fail, but option_writer may add too many
            // options.
            ftl_driver::warn!(env, "failed to finish DHCP packet: {:?}", error);
            return Err(ErrorCode::InvalidArg);
        }

        udp::send_broadcast(env, device, DHCP_CLIENT_PORT, DHCP_SERVER_PORT, &bytes)
    }
}
