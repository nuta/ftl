use alloc::collections::VecDeque;

use ftl_utils::fxhash::FxHashMap;

use crate::device::Tx;
use crate::packet::ipv4::Ipv4Addr;

const MAX_PENDING_TX_QUEUE_DEPTH: usize = 128;

enum ArpEntry<'a> {
    Resolved {
        mac: [u8; 6],
    },
    Pending {
        /// TX packets that are waiting for the ARP response.
        txs: VecDeque<Tx<'a>>,
    },
}

/// A guard struct to enqueue TX packets safely.
pub struct Inserter<'q, 'a> {
    txs: &'q mut VecDeque<Tx<'a>>,
}

impl<'q, 'a> Inserter<'q, 'a> {
    pub fn new(txs: &'q mut VecDeque<Tx<'a>>) -> Option<Self> {
        if txs.len() < MAX_PENDING_TX_QUEUE_DEPTH {
            Some(Self { txs })
        } else {
            None
        }
    }

    pub fn enqueue(self, tx: Tx<'a>) {
        self.txs.push_back(tx);
    }
}

pub struct ArpTable<'a> {
    entries: FxHashMap<Ipv4Addr, ArpEntry<'a>>,
}

impl<'a> ArpTable<'a> {
    pub fn new() -> Self {
        Self {
            entries: FxHashMap::new(),
        }
    }

    pub fn lookup_or_insert(&mut self, ip: Ipv4Addr) -> Result<&[u8; 6], Option<Inserter<'_, 'a>>> {
        let entry = self.entries.entry(ip).or_insert_with(|| {
            ArpEntry::Pending {
                txs: VecDeque::new(),
            }
        });

        match entry {
            ArpEntry::Resolved { mac } => Ok(mac),
            ArpEntry::Pending { txs } => Err(Inserter::new(txs)),
        }
    }

    pub fn resolve(&mut self, ip: Ipv4Addr, mac: [u8; 6]) -> VecDeque<Tx<'a>> {
        match self.entries.insert(ip, ArpEntry::Resolved { mac }) {
            Some(ArpEntry::Pending { txs }) => txs,
            _ => VecDeque::new(),
        }
    }
}
