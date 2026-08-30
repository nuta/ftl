use ftl_driver::dma::DmaBuf;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_driver::net::Notifier;
use ftl_types::error::ErrorCode;
use ftl_types::net::ETHTYPE_ARP;
use ftl_types::net::ETHTYPE_IPV4;
use ftl_utils::spinlock::SpinLock;

use super::arp::ArpTable;
use super::packet::Ipv4Addr;
use crate::shared_ref::SharedRef;

const DRIVER_HEADROOM: usize = 20;
const ETHERNET_HEADROOM: usize = 14;
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

    pub fn ip_header_bytes(&mut self) -> &mut [u8] {
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

    fn write_ethernet_header(&mut self, dst_mac: &[u8; 6], src_mac: &[u8; 6], eth_type: u16) {
        let header = self.ethernet_header_bytes();
        header[..6].copy_from_slice(dst_mac);
        header[6..12].copy_from_slice(src_mac);
        header[12..14].copy_from_slice(&eth_type.to_be_bytes());
    }

    fn write_arp_reply(
        &mut self,
        src_mac: &[u8; 6],
        src_ip: Ipv4Addr,
        dst_mac: &[u8; 6],
        dst_ip: Ipv4Addr,
    ) {
        let arp = self.ip_header_bytes();
        arp[0..2].copy_from_slice(&1u16.to_be_bytes());
        arp[2..4].copy_from_slice(&ETHTYPE_IPV4.to_be_bytes());
        arp[4] = 6;
        arp[5] = 4;
        arp[6..8].copy_from_slice(&2u16.to_be_bytes());
        arp[8..14].copy_from_slice(src_mac);
        arp[14..18].copy_from_slice(&src_ip.as_u32().to_be_bytes());
        arp[18..24].copy_from_slice(dst_mac);
        arp[24..28].copy_from_slice(&dst_ip.as_u32().to_be_bytes());
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

        tx.write_ethernet_header(dst_mac, self.driver.mac_address(), ETHTYPE_IPV4);
        drop(arp_table);
        self.send(env, tx);
        Ok(())
    }

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

    pub fn learn_arp(&self, env: &dyn Env, ip: Ipv4Addr, mac: [u8; 6]) {
        let txs = self.arp_table.lock().resolve(ip, mac);
        for mut tx in txs {
            tx.write_ethernet_header(&mac, self.driver.mac_address(), ETHTYPE_IPV4);
            self.send(env, tx);
        }
    }

    pub fn send_arp_reply(
        &self,
        env: &dyn Env,
        dst_mac: [u8; 6],
        dst_ip: Ipv4Addr,
        our_ip: Ipv4Addr,
    ) {
        let Ok(mut tx) = Tx::alloc(env, 28, 0) else {
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
