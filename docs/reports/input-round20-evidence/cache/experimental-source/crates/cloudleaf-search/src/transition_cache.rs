use crate::Lexicon;
use std::sync::Arc;

const SLOTS: usize = 4096;
const ABSENT: usize = usize::MAX;

#[derive(Clone, Copy)]
struct Slot {
    node: usize,
    next: usize,
    syllable: u32,
}

impl Slot {
    const EMPTY: Self = Self {
        node: ABSENT,
        next: ABSENT,
        syllable: 0,
    };
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TransitionCacheStatistics {
    pub lookups: u64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub invalidations: u64,
    pub resident_entries: usize,
}

/// 只复用不可变词典的音节转移，不缓存输入、候选、解释路径或评分。
pub struct TransitionCache {
    identity: Option<Arc<()>>,
    slots: Vec<Slot>,
    enabled: bool,
    statistics: TransitionCacheStatistics,
}

impl Default for TransitionCache {
    fn default() -> Self {
        Self {
            identity: None,
            slots: Vec::new(),
            enabled: true,
            statistics: TransitionCacheStatistics::default(),
        }
    }
}

impl TransitionCache {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn statistics(&self) -> TransitionCacheStatistics {
        self.statistics
    }

    pub fn capacity_bytes() -> usize {
        std::mem::size_of::<Self>() + SLOTS * std::mem::size_of::<Slot>() + 32
    }

    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.slots.capacity() * std::mem::size_of::<Slot>() + 32
    }

    pub fn clear(&mut self) {
        if self.identity.take().is_some() {
            self.statistics.invalidations = self.statistics.invalidations.saturating_add(1);
        }
        self.slots.clear();
        self.statistics.resident_entries = 0;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.clear();
            self.enabled = enabled;
        }
    }

    pub(crate) fn bind(&mut self, lexicon: &Lexicon) {
        if !self.enabled {
            return;
        }
        if self
            .identity
            .as_ref()
            .is_some_and(|id| !Arc::ptr_eq(id, &lexicon.identity))
        {
            self.clear();
        }
        if self.identity.is_none() {
            self.identity = Some(lexicon.identity.clone());
            self.slots.resize(SLOTS, Slot::EMPTY);
        }
    }

    pub(crate) fn follow(
        &mut self,
        lexicon: &Lexicon,
        node: usize,
        syllable: &str,
    ) -> Option<usize> {
        if !self.enabled {
            return follow(lexicon, node, syllable);
        }
        self.statistics.lookups = self.statistics.lookups.saturating_add(1);
        // 解释器已限定为至多六个小写字母；base27保留长度，编码不会相撞或溢出u32。
        let code = syllable
            .bytes()
            .fold(1u32, |value, byte| value * 27 + u32::from(byte - b'a' + 1));
        let mut hash = (node as u64) ^ u64::from(code).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        hash ^= hash >> 30;
        hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash ^= hash >> 27;
        let slot = &mut self.slots[hash as usize & (SLOTS - 1)];
        if slot.node == node && slot.syllable == code {
            self.statistics.hits = self.statistics.hits.saturating_add(1);
            return (slot.next != ABSENT).then_some(slot.next);
        }
        self.statistics.misses = self.statistics.misses.saturating_add(1);
        let next = follow(lexicon, node, syllable);
        if slot.node == ABSENT {
            self.statistics.resident_entries += 1;
        } else {
            self.statistics.evictions = self.statistics.evictions.saturating_add(1);
        }
        *slot = Slot {
            node,
            next: next.unwrap_or(ABSENT),
            syllable: code,
        };
        next
    }
}

fn follow(lexicon: &Lexicon, mut node: usize, syllable: &str) -> Option<usize> {
    for byte in syllable.bytes() {
        node = *lexicon.trie[node].next.get(&byte)?;
    }
    Some(node)
}
