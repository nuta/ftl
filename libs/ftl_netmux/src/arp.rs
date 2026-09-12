use alloc::collections::VecDeque;

use ftl_utils::fxhash::FxHashMap;
use ftl_utils::fxhash::hash_map::Entry;

use crate::device::Tx;
use crate::packet::ipv4::Ipv4Addr;

const MAX_PENDING_TX_QUEUE_DEPTH: usize = 128;
const MAX_ARP_ENTRIES: usize = 1024;

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
        if txs.len() >= MAX_PENDING_TX_QUEUE_DEPTH {
            return None;
        }

        if txs.try_reserve(1).is_err() {
            return None;
        }

        Some(Self { txs })
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

    pub fn lookup(&mut self, ip: Ipv4Addr) -> Result<&[u8; 6], Option<Inserter<'_, 'a>>> {
        if !self.reserve(true) {
            return Err(None);
        }

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

    pub fn learn(
        &mut self,
        ip: Ipv4Addr,
        mac: [u8; 6],
        evict_on_full: bool,
    ) -> Option<VecDeque<Tx<'a>>> {
        match self.entries.entry(ip) {
            Entry::Occupied(mut entry) => {
                if matches!(entry.get(), ArpEntry::Pending { .. }) {
                    match entry.insert(ArpEntry::Resolved { mac }) {
                        ArpEntry::Pending { txs } => {
                            return Some(txs);
                        }
                        ArpEntry::Resolved { .. } => unreachable!(),
                    }
                }
            }
            Entry::Vacant(_) => {
                if self.reserve(evict_on_full) {
                    self.entries.insert(ip, ArpEntry::Resolved { mac });
                }
            }
        }

        None
    }

    fn reserve(&mut self, evict_on_full: bool) -> bool {
        if self.entries.len() < MAX_ARP_ENTRIES {
            return true;
        }

        if !evict_on_full {
            return false;
        }

        // Remove an entry to make a space.
        if let Some(&ip) = self.entries.keys().next() {
            self.entries.remove(&ip);
        }

        true
    }
}
