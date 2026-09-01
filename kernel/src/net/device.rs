use ftl_driver::dma::DmaBuf;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_driver::net::Notifier;
use ftl_types::error::ErrorCode;
use ftl_types::net::ETHTYPE_ARP;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_utils::spinlock::SpinLock;

use super::arp::ArpTable;
use super::packet::arp::ARP_HW_ETHERNET;
use super::packet::arp::ARP_HWADDR_LEN;
use super::packet::arp::ARP_IPADDR_LEN;
use super::packet::arp::ARP_OP_REPLY;
use super::packet::arp::ArpRewriter;
use super::packet::ethernet::ETHERNET_HEADER_LEN;
use super::packet::ethernet::EthernetRewriter;
use super::packet::ipv4::Ipv4Addr;
use crate::net::GLOBAL_ENV;
use crate::shared_ref::SharedRef;

const DRIVER_HEADROOM: usize = 20;
const ETHERNET_HEADROOM: usize = ETHERNET_HEADER_LEN;
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
            match env.alloc_dma(payload_len) {
                Ok(buf) => Some(buf),
                Err(_) => {
                    env.free_dma(header_buf);
                    return Err(ErrorCode::INVALID_ARG);
                }
            }
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

    pub fn header_and_payload_bytes(&mut self) -> (&mut [u8], Option<&mut [u8]>) {
        let header = &mut self.header_buf.as_mut_slice()[HEADROOM_TOTAL..];
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

    fn write_arp_reply(
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
        tx.write_ethernet_header(dst_mac, self.driver.mac_address(), ETHTYPE_IPV4);
        drop(arp_table);
        self.send(env, tx);
        Ok(())
    }

    pub fn send_ipv4_broadcast(&self, env: &dyn Env, mut tx: Tx) {
        tx.write_ethernet_header(&[u8::MAX; 6], self.driver.mac_address(), ETHTYPE_IPV4);
        self.send(env, tx);
    }

    /// Sends a packet to the driver.
    fn send(&self, env: &dyn Env, tx: Tx) {
        let result = self
            .driver
            .try_send(env, tx.header_buf, DRIVER_HEADROOM, tx.payload_buf);
        if let Err((header_buf, payload_buf, error)) = result {
            warn!("failed to send packet: {:?}", error);
            env.free_dma(header_buf);
            if let Some(payload_buf) = payload_buf {
                env.free_dma(payload_buf);
            }
        }
    }

    pub fn driver(&self) -> &SharedRef<dyn Driver<Notifier = PollNotifier>> {
        &self.driver
    }

    /// Pushes the RX buffer back to the driver.
    pub fn recycle_rx_buffer(&self, buf: DmaBuf) {
        if self.driver.provide(&GLOBAL_ENV, buf).is_err() {
            warn!("failed to recycle an RX buffer");
        }
    }

    /// Fills an ARP table entry.
    pub fn learn_arp(&self, env: &dyn Env, ip: Ipv4Addr, mac: [u8; 6]) {
        let txs = self.arp_table.lock().resolve(ip, mac);
        // Flush pending TX packets.
        for mut tx in txs {
            tx.write_ethernet_header(&mac, self.driver.mac_address(), ETHTYPE_IPV4);
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
        tx.write_ethernet_header(&dst_mac, our_mac, ETHTYPE_ARP);
        tx.write_arp_reply(our_mac, our_ip, &dst_mac, dst_ip);
        self.send(env, tx);
    }
}

pub struct PollNotifier;

impl Notifier for PollNotifier {
    fn notify(&self, _event: ftl_driver::net::Event) {
        // RX is drained by the IRQ handler. Network handles notify their polls.
    }
}
