use ftl_driver::dma::DmaBuf;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_driver::net::Notifier;
use ftl_types::error::ErrorCode;
use ftl_types::net::ETHTYPE_ARP;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_utils::spinlock::SpinLock;

use super::arp::ArpTable;
use super::packet::ARP_HW_ETHERNET;
use super::packet::ARP_HWADDR_LEN;
use super::packet::ARP_IPADDR_LEN;
use super::packet::ARP_OP_REPLY;
use super::packet::ArpRewriter;
use super::packet::EthernetRewriter;
use super::packet::Ipv4Addr;
use crate::shared_ref::SharedRef;

const DRIVER_HEADROOM: usize = 20;
const ETHERNET_HEADROOM: usize = EthernetRewriter::HEADER_LEN;
const HEADROOM_TOTAL: usize = DRIVER_HEADROOM + ETHERNET_HEADROOM;

pub struct Tx {
    header_buf: DmaBuf,
    payload_buf: Option<DmaBuf>,
}

impl Tx {
    pub fn alloc(env: &dyn Env, ip_header_len: usize, payload_len: usize) -> Result<Tx, ErrorCode> {
        // Allocate DMA buffers.
        let header_buf = env
            .alloc_dma(ip_header_len + HEADROOM_TOTAL)
            .map_err(|_| ErrorCode::INVALID_ARG)?;
        let payload_buf = if payload_len == 0 {
            None
        } else {
            Some(
                env.alloc_dma(payload_len)
                    .map_err(|_| ErrorCode::INVALID_ARG)?,
            )
        };

        Ok(Self {
            header_buf,
            payload_buf,
        })
    }

    pub fn header_bytes(&mut self) -> &mut [u8] {
        &mut self.header_buf.as_mut_slice()[HEADROOM_TOTAL..]
    }

    fn ethernet_header_bytes(&mut self) -> &mut [u8] {
        &mut self.header_buf.as_mut_slice()[DRIVER_HEADROOM..DRIVER_HEADROOM + ETHERNET_HEADROOM]
    }

    pub fn payload_bytes(&mut self) -> Option<&mut [u8]> {
        self.payload_buf
            .as_mut()
            .map(|payload_buf| payload_buf.as_mut_slice())
    }

    fn rewrite_ethernet_header(&mut self, dst_mac: &[u8; 6], src_mac: &[u8; 6], eth_type: u16) {
        let mut ethernet = EthernetRewriter::new(self.ethernet_header_bytes()).unwrap();
        ethernet.set_dst_mac(*dst_mac);
        ethernet.set_src_mac(*src_mac);
        ethernet.set_eth_type(eth_type);
    }

    fn rewrite_arp_reply(
        &mut self,
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
        arp.set_operation(ARP_OP_REPLY);
        arp.set_src_mac(*src_mac);
        arp.set_src_ip(src_ip);
        arp.set_dst_mac(*dst_mac);
        arp.set_dst_ip(dst_ip);
    }
}

pub struct Device {
    driver: SharedRef<dyn Driver<Notifier = PollNotifier>>,
    arp_table: SpinLock<ArpTable>,
}

impl Device {
    pub fn new(driver: SharedRef<dyn Driver<Notifier = PollNotifier>>) -> Self {
        Self {
            driver,
            arp_table: SpinLock::new(ArpTable::new()),
        }
    }

    pub fn send_ipv4(
        &self,
        env: &dyn Env,
        next_hop_ip: Ipv4Addr,
        mut tx: Tx,
    ) -> Result<(), ErrorCode> {
        let mut arp_table = self.arp_table.lock();
        let dst_mac = match arp_table.lookup_or_insert(next_hop_ip) {
            Ok(dst_mac) => dst_mac,
            Err(inserter) => {
                inserter.enqueue(tx);
                return Ok(());
            }
        };

        // Fill the ethernet header and send it to the driver.
        tx.rewrite_ethernet_header(dst_mac, self.driver.mac_address(), ETHTYPE_IPV4);
        drop(arp_table);
        self.send(env, tx);
        Ok(())
    }

    /// Sends a packet to the driver.
    fn send(&self, env: &dyn Env, tx: Tx) {
        let result = self
            .driver
            .try_send(env, tx.header_buf, DRIVER_HEADROOM, tx.payload_buf);
        if let Err((_, _, error)) = result {
            warn!("net: failed to send packet: {:?}", error);
        }
    }

    pub fn driver(&self) -> &SharedRef<dyn Driver<Notifier = PollNotifier>> {
        &self.driver
    }

    /// Fills an ARP table entry.
    pub fn learn_arp(&self, env: &dyn Env, ip: Ipv4Addr, mac: [u8; 6]) {
        let txs = self.arp_table.lock().resolve(ip, mac);
        // Flush pending TX packets.
        for mut tx in txs {
            tx.rewrite_ethernet_header(&mac, self.driver.mac_address(), ETHTYPE_IPV4);
            self.send(env, tx);
        }
    }

    /// Sends an ARP reply.
    pub fn send_arp_reply(
        &self,
        env: &dyn Env,
        dst_mac: [u8; 6],
        dst_ip: Ipv4Addr,
        our_ip: Ipv4Addr,
    ) {
        let Ok(mut tx) = Tx::alloc(env, ArpRewriter::PACKET_LEN, 0) else {
            return;
        };

        let our_mac = self.driver.mac_address();
        tx.rewrite_ethernet_header(&dst_mac, our_mac, ETHTYPE_ARP);
        tx.rewrite_arp_reply(our_mac, our_ip, &dst_mac, dst_ip);
        self.send(env, tx);
    }
}

pub struct PollNotifier;

impl Notifier for PollNotifier {
    fn notify(&self, _event: ftl_driver::net::Event) {
        // RX is drained by the IRQ handler. Network handles notify their polls.
    }
}
