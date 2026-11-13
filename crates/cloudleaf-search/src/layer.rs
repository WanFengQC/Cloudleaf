//! 用户层共享基础数组，只复制发生变化的Trie节点；构建不在按键路径执行。
use crate::lexicon::{Entry, FrequencyKind, TrieNode};
use crate::{ErrorKind, Lexicon, LexiconMemory, SearchError};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Index,
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct Entries {
    base: Arc<Vec<Entry>>,
    extra: Vec<Entry>,
}
impl PartialEq for Entries {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl Eq for Entries {}
impl Entries {
    pub(crate) fn new(base: Vec<Entry>) -> Self {
        Self {
            base: Arc::new(base),
            extra: Vec::new(),
        }
    }
    pub fn len(&self) -> usize {
        self.base.len() + self.extra.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn iter(
        &self,
    ) -> std::iter::Chain<std::slice::Iter<'_, Entry>, std::slice::Iter<'_, Entry>> {
        self.base.iter().chain(self.extra.iter())
    }
    pub(crate) fn capacity(&self) -> usize {
        self.base.capacity() + self.extra.capacity()
    }
}
impl Index<usize> for Entries {
    type Output = Entry;
    fn index(&self, i: usize) -> &Entry {
        if i < self.base.len() {
            &self.base[i]
        } else {
            &self.extra[i - self.base.len()]
        }
    }
}
impl<'a> IntoIterator for &'a Entries {
    type Item = &'a Entry;
    type IntoIter = std::iter::Chain<std::slice::Iter<'a, Entry>, std::slice::Iter<'a, Entry>>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Trie {
    base: Arc<Vec<TrieNode>>,
    changed: BTreeMap<usize, TrieNode>,
    extra: Vec<TrieNode>,
}
impl Trie {
    pub(crate) fn new(base: Vec<TrieNode>) -> Self {
        Self {
            base: Arc::new(base),
            changed: BTreeMap::new(),
            extra: Vec::new(),
        }
    }
    pub(crate) fn len(&self) -> usize {
        self.base.len() + self.extra.len()
    }
    pub(crate) fn capacity(&self) -> usize {
        self.base.capacity() + self.extra.capacity()
    }
    fn node_mut(&mut self, i: usize) -> &mut TrieNode {
        if i < self.base.len() {
            self.changed
                .entry(i)
                .or_insert_with(|| self.base[i].clone())
        } else {
            &mut self.extra[i - self.base.len()]
        }
    }
}
impl Index<usize> for Trie {
    type Output = TrieNode;
    fn index(&self, i: usize) -> &TrieNode {
        if i < self.base.len() {
            self.changed.get(&i).unwrap_or(&self.base[i])
        } else {
            &self.extra[i - self.base.len()]
        }
    }
}
impl<'a> IntoIterator for &'a Trie {
    type Item = &'a TrieNode;
    type IntoIter =
        std::iter::Map<std::ops::Range<usize>, Box<dyn FnMut(usize) -> &'a TrieNode + 'a>>;
    fn into_iter(self) -> Self::IntoIter {
        (0..self.len()).map(Box::new(move |i| &self[i]))
    }
}

impl Lexicon {
    /// 此处是独立用户包进入搜索索引的边界；私有计数不是来源词频。
    pub fn with_user_layer(
        &self,
        text: &str,
        preferences: &BTreeMap<String, u32>,
    ) -> Result<Self, SearchError> {
        let invalid = || SearchError::new(ErrorKind::InvalidLexicon, "用户层来源、身份或容量无效");
        if self.base_memory.is_some()
            || text.len() > 4 * 1024 * 1024
            || preferences.values().any(|v| *v > 8)
        {
            return Err(invalid());
        }
        let users = Self::from_tsv_with_limits(
            text,
            &crate::LoadLimits {
                max_bytes: 4 * 1024 * 1024,
                max_entries: 4096,
                max_trie_nodes: 400000,
            },
        )?;
        let existing: BTreeSet<_> = self.entries.iter().map(|e| e.id.as_str()).collect();
        if users.entries.iter().any(|e| {
            !e.id.starts_with("user:")
                || e.source != "user"
                || e.source_record_id != e.id
                || e.license_id != "LicenseRef-User-Private"
                || e.frequency_kind != FrequencyKind::Unknown
                || existing.contains(e.id.as_str())
        }) {
            return Err(invalid());
        }
        let mut result = self.clone();
        result.base_memory = Some(self.memory_estimate());
        result.identity = Arc::new(());
        for entry in users.entries.iter() {
            let index = result.entries.len();
            let last_start = entry.boundaries.last().copied().unwrap_or(0);
            let mut node = 0;
            for (offset, byte) in entry.letters.iter().enumerate() {
                node = if let Some(&next) = result.trie[node].next.get(byte) {
                    next
                } else {
                    let next = result.trie.len();
                    result.trie.extra.push(TrieNode::default());
                    result.trie.node_mut(node).next.insert(*byte, next);
                    next
                };
                if offset + 1 > last_start && offset + 1 < entry.letters.len() {
                    result.trie.node_mut(node).prefix.push(index)
                }
            }
            result.trie.node_mut(node).exact.push(index);
            result.entries.extra.push(entry.clone());
        }
        let mut known = 0;
        let units: Vec<_> = result
            .entries
            .iter()
            .map(|e| {
                let learned = preferences.get(&e.id).copied().unwrap_or(0);
                if preferences.contains_key(&e.id) {
                    known += 1
                }
                learned + u32::from(e.source == "user")
            })
            .collect();
        if known != preferences.len() {
            return Err(invalid());
        }
        result.preferences = units.iter().any(|u| *u != 0).then_some(units);
        Ok(result)
    }

    pub fn user_preference(&self, index: usize) -> u32 {
        self.preferences.as_ref().map_or(0, |v| v[index])
    }
    pub fn has_user_preferences(&self) -> bool {
        self.preferences.is_some()
    }
    pub fn shares_base_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.entries.base, &other.entries.base)
            && Arc::ptr_eq(&self.trie.base, &other.trie.base)
    }
    pub fn is_extension_of(&self, other: &Self) -> bool {
        self.shares_base_with(other) && other.base_memory.is_none()
    }
    pub fn incremental_estimated_bytes(&self) -> usize {
        if self.base_memory.is_none() {
            return self.memory_estimate().total_estimated_bytes;
        }
        let entries = self.entries.extra.capacity() * std::mem::size_of::<Entry>()
            + self
                .entries
                .extra
                .iter()
                .map(|e| {
                    e.id.capacity()
                        + e.pinyin.capacity()
                        + e.text.capacity()
                        + e.annotation.capacity()
                        + e.source.capacity()
                        + e.source_record_id.capacity()
                        + e.license_id.capacity()
                        + e.letters.capacity()
                        + 512
                })
                .sum::<usize>();
        let nodes = self
            .trie
            .changed
            .values()
            .chain(self.trie.extra.iter())
            .map(|n| {
                std::mem::size_of::<TrieNode>()
                    + (n.exact.capacity() + n.prefix.capacity()) * std::mem::size_of::<usize>()
                    + 1024
            })
            .sum::<usize>();
        let map = (self.trie.changed.len().div_ceil(5) + 1)
            * (32 + 12 * 8 + 11 * std::mem::size_of::<(usize, TrieNode)>());
        entries
            + nodes
            + map
            + (self.trie.extra.capacity() - self.trie.extra.len()) * std::mem::size_of::<TrieNode>()
            + self.preferences.as_ref().map_or(0, |v| v.capacity() * 4)
            + std::mem::size_of::<Self>()
            + 64
    }
    pub(crate) fn layered_memory(&self) -> Option<LexiconMemory> {
        self.base_memory.map(|m| {
            let extra = self.incremental_estimated_bytes();
            LexiconMemory {
                owned_capacity_bytes: m.owned_capacity_bytes + extra,
                btree_node_estimate_bytes: m.btree_node_estimate_bytes,
                total_estimated_bytes: m.total_estimated_bytes + extra,
            }
        })
    }
}
