use alloc::sync::Arc;

use ftl::time::MonoTime;
use hashbrown::HashMap;

use super::conn::TcpConn;
use super::packet::TcpPacketInfo;
use crate::net::ipv4::Endpoint;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TcpFlowKey {
    local_port: u16,
    remote_ip: u32,
    remote_port: u16,
}

impl TcpFlowKey {
    fn from_packet(pkt: &TcpPacketInfo) -> Self {
        Self {
            local_port: pkt.local_port,
            remote_ip: pkt.remote_ip,
            remote_port: pkt.remote_port,
        }
    }
}

/// Maps TCP flows to their state.
pub struct TcpFlows {
    flows: HashMap<TcpFlowKey, Arc<TcpConn>>,
}

impl TcpFlows {
    pub fn new() -> Self {
        Self {
            flows: HashMap::new(),
        }
    }

    pub fn insert(&mut self, local_port: u16, remote: Endpoint, conn: Arc<TcpConn>) {
        let key = TcpFlowKey {
            local_port,
            remote_ip: remote.ip,
            remote_port: remote.port,
        };
        self.flows.insert(key, conn);
    }

    pub fn contains(&self, local_port: u16, remote: Endpoint) -> bool {
        let key = TcpFlowKey {
            local_port,
            remote_ip: remote.ip,
            remote_port: remote.port,
        };
        self.flows.contains_key(&key)
    }

    pub fn lookup(&self, pkt: &TcpPacketInfo) -> Option<Arc<TcpConn>> {
        let key = TcpFlowKey::from_packet(pkt);
        self.flows.get(&key).cloned()
    }

    pub fn poll(&self, now: MonoTime) {
        for conn in self.flows.values() {
            conn.poll(now);
        }
    }

    // TODO: Optimize this.
    pub fn pop_closed(&mut self) -> Option<u16> {
        let mut to_remove = None;
        for (key, conn) in self.flows.iter() {
            if conn.is_closed() {
                to_remove = Some(*key);
                break;
            }
        }

        let key = to_remove?;
        self.flows.remove(&key);
        Some(key.local_port)
    }
}
