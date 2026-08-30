use alloc::collections::VecDeque;

use hashbrown::HashMap;

use super::device::Tx;
use super::packet::Ipv4Addr;

enum ArpEntry {
    Resolved {
        mac: [u8; 6],
    },
    Pending {
        /// TX packets that are waiting for the ARP response.
        txs: VecDeque<Tx>,
    },
}

/// A guard struct to enqueue TX packets safely.
pub struct Inserter<'a> {
    txs: &'a mut VecDeque<Tx>,
}

impl<'a> Inserter<'a> {
    pub fn enqueue(self, tx: Tx) {
        self.txs.push_back(tx);
    }
}

pub struct ArpTable {
    entries: HashMap<Ipv4Addr, ArpEntry>,
}

impl ArpTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn lookup_or_insert(&mut self, ip: Ipv4Addr) -> Result<&[u8; 6], Inserter<'_>> {
        let entry = self.entries.entry(ip).or_insert_with(|| {
            ArpEntry::Pending {
                txs: VecDeque::new(),
            }
        });

        match entry {
            ArpEntry::Resolved { mac } => Ok(mac),
            ArpEntry::Pending { txs } => Err(Inserter { txs }),
        }
    }

    pub fn resolve(&mut self, ip: Ipv4Addr, mac: [u8; 6]) -> VecDeque<Tx> {
        match self.entries.insert(ip, ArpEntry::Resolved { mac }) {
            Some(ArpEntry::Pending { txs }) => txs,
            _ => VecDeque::new(),
        }
    }
}
