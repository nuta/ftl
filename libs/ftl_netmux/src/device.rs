use ftl_driver::dma::DmaBuf;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_driver::net::Notifier;
use ftl_types::error::ErrorCode;
use ftl_types::net::ETHTYPE_ARP;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_utils::spinlock::SpinLock;

use crate::arp::ArpTable;
use crate::dhcp::Client;
use crate::dhcp::DhcpConfig;
use crate::packet::arp::ARP_HW_ETHERNET;
use crate::packet::arp::ARP_HWADDR_LEN;
use crate::packet::arp::ARP_IPADDR_LEN;
use crate::packet::arp::ARP_OP_REPLY;
use crate::packet::arp::ARP_OP_REQUEST;
use crate::packet::arp::ArpRewriter;
use crate::packet::ethernet::ETHERNET_HEADER_LEN;
use crate::packet::ethernet::EthernetRewriter;
use crate::packet::ipv4::Ipv4Addr;
use crate::packet::ipv4::Ipv4Inspector;
use crate::packet::udp::UdpInspector;

const DRIVER_HEADROOM: usize = 20;
const ETHERNET_HEADROOM: usize = ETHERNET_HEADER_LEN;
const HEADROOM_TOTAL: usize = DRIVER_HEADROOM + ETHERNET_HEADROOM;

pub struct Tx<'a> {
    env: &'a dyn Env,
    header_buf: Option<DmaBuf>,
    payload_buf: Option<DmaBuf>,
}

impl<'a> Tx<'a> {
    pub fn alloc(
        env: &'a dyn Env,
        ip_header_len: usize,
        payload_len: usize,
    ) -> Result<Tx<'a>, ErrorCode> {
        // Allocate DMA buffers.
        let header_buf = env
            .alloc_dma(ip_header_len + HEADROOM_TOTAL)
            .map_err(|_| ErrorCode::OutOfMemory)?;

        let payload_buf = if payload_len == 0 {
            None
        } else {
            match env.alloc_dma(payload_len) {
                Ok(buf) => Some(buf),
                Err(_) => {
                    env.free_dma(header_buf);
                    return Err(ErrorCode::OutOfMemory);
                }
            }
        };

        Ok(Self {
            env,
            header_buf: Some(header_buf),
            payload_buf,
        })
    }

    pub fn header_bytes(&mut self) -> &mut [u8] {
        &mut self.header_buf.as_mut().unwrap().as_mut_slice()[HEADROOM_TOTAL..]
    }

    fn ethernet_header_bytes(&mut self) -> &mut [u8] {
        &mut self.header_buf.as_mut().unwrap().as_mut_slice()
            [DRIVER_HEADROOM..DRIVER_HEADROOM + ETHERNET_HEADROOM]
    }

    pub fn header_and_payload_bytes(&mut self) -> (&mut [u8], Option<&mut [u8]>) {
        let header = &mut self.header_buf.as_mut().unwrap().as_mut_slice()[HEADROOM_TOTAL..];
        let payload = self
            .payload_buf
            .as_mut()
            .map(|payload_buf| payload_buf.as_mut_slice());
        (header, payload)
    }

    fn write_ethernet_header(&mut self, dst_mac: &[u8; 6], src_mac: &[u8; 6], eth_type: u16) {
        let mut ethernet = EthernetRewriter::new(self.ethernet_header_bytes()).unwrap();
        ethernet.set_dst_mac(*dst_mac);
        ethernet.set_src_mac(*src_mac);
        ethernet.set_eth_type(eth_type);
    }

    fn write_arp(
        &mut self,
        op: u16,
        src_mac: &[u8; 6],
        src_ip: Ipv4Addr,
        dst_mac: &[u8; 6],
        dst_ip: Ipv4Addr,
    ) {
        let mut arp = ArpRewriter::new(self.header_bytes()).unwrap();
        arp.set_hardware_type(ARP_HW_ETHERNET);
        arp.set_protocol_type(ETHTYPE_IPV4);
        arp.set_hardware_addr_len(ARP_HWADDR_LEN);
        arp.set_protocol_addr_len(ARP_IPADDR_LEN);
        arp.set_operation(op);
        arp.set_src_mac(*src_mac);
        arp.set_src_ip(src_ip);
        arp.set_dst_mac(*dst_mac);
        arp.set_dst_ip(dst_ip);
    }
}

impl Drop for Tx<'_> {
    fn drop(&mut self) {
        if let Some(buf) = self.header_buf.take() {
            self.env.free_dma(buf);
        }
        if let Some(buf) = self.payload_buf.take() {
            self.env.free_dma(buf);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(u64);

impl DeviceId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

pub struct Device<'a> {
    env: &'a dyn Env,
    driver: &'a dyn Driver<Notifier = PollNotifier>,
    arp_table: SpinLock<ArpTable<'a>>,
    dhcp: Option<Client>,
}

impl<'a> Device<'a> {
    pub fn new(env: &'a dyn Env, driver: &'a dyn Driver<Notifier = PollNotifier>) -> Self {
        Self {
            env,
            driver,
            arp_table: SpinLock::new(ArpTable::new()),
            dhcp: None,
        }
    }

    pub fn send_ipv4(
        &self,
        our_ip: Ipv4Addr,
        next_hop_ip: Ipv4Addr,
        mut tx: Tx<'a>,
    ) -> Result<(), ErrorCode> {
        // Look up the destination MAC address in the ARP table.
        let mut arp_table = self.arp_table.lock();
        let dst_mac = match arp_table.lookup(next_hop_ip) {
            Ok(dst_mac) => dst_mac,
            Err(Some(inserter)) => {
                // We don't know the destination MAC address.
                inserter.enqueue(tx);
                drop(arp_table);

                // Send an ARP request.
                let mut request = Tx::alloc(self.env, ArpRewriter::PACKET_LEN, 0)?;
                let our_mac = self.driver.mac_address();
                request.write_ethernet_header(&[0xff; 6], our_mac, ETHTYPE_ARP);
                request.write_arp(ARP_OP_REQUEST, our_mac, our_ip, &[0; 6], next_hop_ip);
                self.send(request)?;

                // We'll send the enqueued packet later. Finish successfully.
                return Ok(());
            }
            Err(None) => {
                // The queue in the ARP table is full. Drop the packet.
                ftl_driver::trace!(self.env, "dropped a packet because the ARP table is full");
                drop(tx);
                return Ok(());
            }
        };

        // Fill the ethernet header and send it to the driver.
        tx.write_ethernet_header(dst_mac, self.driver.mac_address(), ETHTYPE_IPV4);
        drop(arp_table);
        self.send(tx)
    }

    pub fn send_ipv4_broadcast(&self, mut tx: Tx<'a>) -> Result<(), ErrorCode> {
        tx.write_ethernet_header(&[u8::MAX; 6], self.driver.mac_address(), ETHTYPE_IPV4);
        self.send(tx)
    }

    /// Sends a packet to the driver.
    fn send(&self, mut tx: Tx<'a>) -> Result<(), ErrorCode> {
        let result = self.driver.try_send(
            self.env,
            tx.header_buf.take().unwrap(),
            DRIVER_HEADROOM,
            tx.payload_buf.take(),
        );

        if let Err((header_buf, payload_buf, error)) = result {
            ftl_driver::warn!(self.env, "failed to send packet: {:?}", error);

            // Free DMA buffers returned back by the driver.
            self.env.free_dma(header_buf);
            if let Some(payload_buf) = payload_buf {
                self.env.free_dma(payload_buf);
            }

            return Err(ErrorCode::InvalidState);
        }

        Ok(())
    }

    pub fn driver(&self) -> &'a dyn Driver<Notifier = PollNotifier> {
        self.driver
    }

    /// Fills an ARP table entry.
    pub fn learn_arp(&self, ip: Ipv4Addr, mac: [u8; 6]) {
        if let Some(txs) = self.arp_table.lock().learn(ip, mac, false) {
            // Flush pending TX packets.
            for mut tx in txs {
                tx.write_ethernet_header(&mac, self.driver.mac_address(), ETHTYPE_IPV4);
                if let Err(error) = self.send(tx) {
                    ftl_driver::warn!(self.env, "failed to send pending IPv4 packet: {:?}", error);
                }
            }
        }
    }

    /// Sends an ARP reply.
    pub fn send_arp_reply(&self, dst_mac: [u8; 6], dst_ip: Ipv4Addr, our_ip: Ipv4Addr) {
        let Ok(mut tx) = Tx::alloc(self.env, ArpRewriter::PACKET_LEN, 0) else {
            return;
        };

        let our_mac = self.driver.mac_address();
        tx.write_ethernet_header(&dst_mac, our_mac, ETHTYPE_ARP);
        tx.write_arp(ARP_OP_REPLY, our_mac, our_ip, &dst_mac, dst_ip);
        if let Err(error) = self.send(tx) {
            ftl_driver::warn!(self.env, "failed to send ARP reply: {:?}", error);
        }
    }

    pub(crate) fn start_dhcp(&mut self) {
        let mac = *self.driver.mac_address();
        let client = Client::new(mac);
        if let Err(error) = client.send_discover(self.env, self) {
            ftl_driver::warn!(self.env, "failed to send DHCP discover: {:?}", error);
        }
        self.dhcp = Some(client);
    }

    pub(crate) fn handle_dhcp_rx(
        &mut self,
        ipv4: &Ipv4Inspector<'_>,
        udp: &UdpInspector<'_>,
    ) -> Option<DhcpConfig> {
        let mut client = self.dhcp.take()?;
        let result = client.handle_rx(self.env, self, ipv4, udp);
        self.dhcp = Some(client);
        result
    }
}

pub struct PollNotifier;

impl Notifier for PollNotifier {
    fn notify(&self, _event: ftl_driver::net::Event) {
        // RX is drained by the IRQ handler. Network handles notify their polls.
    }
}
