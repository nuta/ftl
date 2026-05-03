use crate::ethernet::EthernetHeader;
use crate::ip::Ipv4Addr;
use crate::ip::ipv4::Ipv4Header;
use crate::packet;
use crate::packet::Packet;
use crate::transport::Port;
use crate::udp::UdpHeader;

pub(crate) struct Tx {
    pub local_ip: Ipv4Addr,
    pub local_port: Port,
    pub remote_ip: Ipv4Addr,
    pub remote_port: Port,
    pub pkt: Packet,
}

#[derive(Debug)]
pub(crate) enum TxError {
    PacketAlloc(packet::AllocError),
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
}

impl DhcpClient {
    pub fn new() -> Self {
        Self {
            state: spin::Mutex::new(State::Init),
        }
    }

    pub fn poll_tx(
        &mut self,
    ) -> Result<Option<Tx>, TxError> {
        let state = self.state.lock();
        match *state {
            State::Init => {
                let head_room = size_of::<EthernetHeader>() + size_of::<Ipv4Header>() + size_of::<UdpHeader>();
                let pkt = Packet::new(0, head_room).map_err(TxError::PacketAlloc)?;
                Ok(Some(Tx {
                    local_ip: Ipv4Addr::UNSPECIFIED,
                    local_port: Port::new(68),
                    remote_ip: Ipv4Addr::BROADCAST,
                    remote_port: Port::new(67),
                    pkt,
                }))
            }
            State::SentDiscover => {
                Ok(None)
            }
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
