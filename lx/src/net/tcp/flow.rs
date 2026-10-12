use alloc::sync::Arc;

use ftl::time::MonoTime;
use ftl_types::net::Rule;
use hashbrown::HashMap;

use super::conn::TcpConn;
use super::packet::TcpPacketInfo;
use crate::net::ipv4::Endpoint;

struct TcpFlow {
    conn: Arc<TcpConn>,
    rule: Rule,
}

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
    flows: HashMap<TcpFlowKey, TcpFlow>,
}

impl TcpFlows {
    pub fn new() -> Self {
        Self {
            flows: HashMap::new(),
        }
    }

    pub fn insert(&mut self, local_port: u16, remote: Endpoint, conn: Arc<TcpConn>, rule: Rule) {
        let key = TcpFlowKey {
            local_port,
            remote_ip: remote.ip,
            remote_port: remote.port,
        };
        self.flows.insert(key, TcpFlow { conn, rule });
    }

    pub fn lookup(&self, pkt: &TcpPacketInfo) -> Option<Arc<TcpConn>> {
        let key = TcpFlowKey::from_packet(pkt);
        self.flows.get(&key).map(|flow| flow.conn.clone())
    }

    pub fn poll(&self, now: MonoTime) {
        for flow in self.flows.values() {
            flow.conn.poll(now);
        }
    }

    // TODO: Optimize this.
    pub fn pop_closed(&mut self) -> Option<Rule> {
        let mut to_remove = None;
        for (tuple, flow) in self.flows.iter_mut() {
            if flow.conn.is_closed() {
                to_remove = Some((*tuple, flow.rule));
                break;
            }
        }

        if let Some((tuple, rule)) = to_remove {
            self.flows.remove(&tuple);
            Some(rule)
        } else {
            None
        }
    }
}
