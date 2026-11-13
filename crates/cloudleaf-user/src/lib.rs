//! 平台无关用户词库管理，不在按键路径执行文件操作。
mod store;
pub use store::{SaveFault, Store};

use ring::digest::{SHA256, digest};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 4_096;
pub const MAX_PREFERENCES: usize = 10_000;
pub const MAX_COUNT: u32 = 1_000_000;
const RECENT_EVENTS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    InvalidData,
    UnsupportedVersion,
    Limit,
    Conflict,
    Busy,
    Io,
    RecoveryRequired,
    Unknown,
    Untrusted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    pub os_code: Option<i32>,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            os_code: None,
        }
    }
    fn io(error: std::io::Error) -> Self {
        Self {
            kind: ErrorKind::Io,
            os_code: error.raw_os_error(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            ErrorKind::InvalidData => "用户词库记录或校验无效",
            ErrorKind::UnsupportedVersion => "用户词库版本不兼容",
            ErrorKind::Limit => "用户词库超过容量限制",
            ErrorKind::Conflict => "用户词库版本、身份或导入内容冲突",
            ErrorKind::Busy => "另一实例正在管理用户词库",
            ErrorKind::Io => "用户词库文件操作失败，原内存内容保留",
            ErrorKind::RecoveryRequired => "用户词库需要显式恢复，原件保留",
            ErrorKind::Unknown => "保存结果未知，请重新加载核对，禁止盲目重试",
            ErrorKind::Untrusted => "模拟来源不能写入学习统计",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Eq, PartialEq)]
pub struct UserEntry {
    pub id: String,
    pub revision: u64,
    pub enabled: bool,
    pub text: String,
    pub pinyin: String,
}

impl fmt::Debug for UserEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserEntry")
            .field("id", &self.id)
            .field("revision", &self.revision)
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionOrigin {
    IsolatedOwnedWindow,
    NativeTsf,
    Simulation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionEvent {
    pub token: String,
    pub origin: SelectionOrigin,
    pub learning_epoch: u64,
    pub entries: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct UserDictionary {
    generation: u64,
    next_id: u64,
    learning_enabled: bool,
    learning_epoch: u64,
    entries: BTreeMap<String, UserEntry>,
    preferences: BTreeMap<(String, String), u32>,
    recent_events: VecDeque<String>,
    purge_history: bool,
}

impl PartialEq for UserDictionary {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.next_id == other.next_id
            && self.learning_enabled == other.learning_enabled
            && self.learning_epoch == other.learning_epoch
            && self.entries == other.entries
            && self.preferences == other.preferences
    }
}
impl Eq for UserDictionary {}

impl fmt::Debug for UserDictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserDictionary")
            .field("generation", &self.generation)
            .field("entries", &self.entries.len())
            .field("preferences", &self.preferences.len())
            .field("learning_enabled", &self.learning_enabled)
            .finish_non_exhaustive()
    }
}

impl Default for UserDictionary {
    fn default() -> Self {
        Self::new()
    }
}

impl UserDictionary {
    pub fn new() -> Self {
        Self {
            generation: 0,
            next_id: 1,
            learning_enabled: false,
            learning_epoch: 1,
            entries: BTreeMap::new(),
            preferences: BTreeMap::new(),
            recent_events: VecDeque::new(),
            purge_history: false,
        }
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn learning_enabled(&self) -> bool {
        self.learning_enabled
    }
    pub fn learning_epoch(&self) -> u64 {
        self.learning_epoch
    }
    pub fn entries(&self) -> &BTreeMap<String, UserEntry> {
        &self.entries
    }
    pub fn get(&self, id: &str) -> Option<&UserEntry> {
        self.entries.get(id)
    }
    pub fn preferences(&self) -> &BTreeMap<(String, String), u32> {
        &self.preferences
    }
    /// 固定Rust1.90容器布局的保守容量估计，不是RSS或分配器硬上界。
    pub fn estimated_bytes(&self) -> usize {
        let nodes = |len: usize, item: usize| (len.div_ceil(5) + 1) * (32 + 12 * 8 + 11 * item);
        std::mem::size_of::<Self>()
            + self
                .entries
                .iter()
                .map(|(id, entry)| {
                    id.capacity()
                        + entry.id.capacity()
                        + entry.text.capacity()
                        + entry.pinyin.capacity()
                })
                .sum::<usize>()
            + self
                .preferences
                .keys()
                .map(|(id, fp)| id.capacity() + fp.capacity())
                .sum::<usize>()
            + nodes(
                self.entries.len(),
                std::mem::size_of::<(String, UserEntry)>(),
            )
            + nodes(
                self.preferences.len(),
                std::mem::size_of::<((String, String), u32)>(),
            )
            + self.recent_events.capacity() * std::mem::size_of::<String>()
            + self
                .recent_events
                .iter()
                .map(String::capacity)
                .sum::<usize>()
    }
    pub fn preference(&self, id: &str, fingerprint: &str) -> u32 {
        self.preferences
            .get(&(id.into(), fingerprint.into()))
            .copied()
            .unwrap_or(0)
    }

    pub fn add(&mut self, text: &str, pinyin: &str) -> Result<String> {
        validate_pair(text, pinyin)?;
        self.transaction(|next| {
            if next.entries.len() >= MAX_ENTRIES {
                return Err(Error::new(ErrorKind::Limit));
            }
            if next
                .entries
                .values()
                .any(|e| e.text == text && e.pinyin == pinyin)
            {
                return Err(Error::new(ErrorKind::Conflict));
            }
            let id = format!("user:{}", next.next_id);
            next.next_id = next
                .next_id
                .checked_add(1)
                .ok_or(Error::new(ErrorKind::Limit))?;
            next.entries.insert(
                id.clone(),
                UserEntry {
                    id: id.clone(),
                    revision: 1,
                    enabled: true,
                    text: text.into(),
                    pinyin: pinyin.into(),
                },
            );
            next.invalidate_selection_epoch()?;
            Ok(id)
        })
    }

    pub fn update(&mut self, id: &str, text: &str, pinyin: &str) -> Result<()> {
        validate_pair(text, pinyin)?;
        self.transaction(|next| {
            if next
                .entries
                .values()
                .any(|e| e.id != id && e.text == text && e.pinyin == pinyin)
            {
                return Err(Error::new(ErrorKind::Conflict));
            }
            let entry = next
                .entries
                .get_mut(id)
                .ok_or(Error::new(ErrorKind::Conflict))?;
            entry.revision = entry
                .revision
                .checked_add(1)
                .ok_or(Error::new(ErrorKind::Limit))?;
            entry.text = text.into();
            entry.pinyin = pinyin.into();
            next.preferences.retain(|(key, _), _| key != id);
            next.purge_history = true;
            next.invalidate_selection_epoch()?;
            Ok(())
        })
    }

    pub fn remove(&mut self, id: &str) -> Result<()> {
        self.transaction(|next| {
            if next.entries.remove(id).is_none() {
                return Err(Error::new(ErrorKind::Conflict));
            }
            next.preferences.retain(|(key, _), _| key != id);
            next.purge_history = true;
            next.invalidate_selection_epoch()?;
            Ok(())
        })
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<()> {
        self.transaction(|next| {
            let entry = next
                .entries
                .get_mut(id)
                .ok_or(Error::new(ErrorKind::Conflict))?;
            if entry.enabled != enabled {
                entry.revision = entry
                    .revision
                    .checked_add(1)
                    .ok_or(Error::new(ErrorKind::Limit))?;
                entry.enabled = enabled;
                next.invalidate_selection_epoch()?;
            }
            Ok(())
        })
    }

    pub fn clear_entries(&mut self) -> Result<()> {
        self.transaction(|next| {
            next.entries.clear();
            next.preferences
                .retain(|(id, _), _| !id.starts_with("user:"));
            next.purge_history = true;
            next.invalidate_selection_epoch()?;
            Ok(())
        })
    }

    pub fn set_learning_enabled(&mut self, enabled: bool) -> Result<()> {
        self.transaction(|next| {
            if next.learning_enabled != enabled {
                next.learning_epoch = next
                    .learning_epoch
                    .checked_add(1)
                    .ok_or(Error::new(ErrorKind::Limit))?;
                next.learning_enabled = enabled;
                next.recent_events.clear();
            }
            Ok(())
        })
    }

    pub fn clear_learning(&mut self) -> Result<()> {
        self.transaction(|next| {
            next.learning_epoch = next
                .learning_epoch
                .checked_add(1)
                .ok_or(Error::new(ErrorKind::Limit))?;
            next.preferences.clear();
            next.recent_events.clear();
            next.purge_history = true;
            Ok(())
        })
    }

    /// 调用者仍须在可信宿主边界核对Applied、会话、revision及来源；模拟成功不获得写入资格。
    pub fn record_selection(&mut self, event: &SelectionEvent) -> Result<bool> {
        if event.origin == SelectionOrigin::Simulation {
            return Err(Error::new(ErrorKind::Untrusted));
        }
        if !identifier(&event.token)
            || event.entries.is_empty()
            || event.entries.len() > 64
            || event
                .entries
                .iter()
                .any(|(id, fp)| !identifier(id) || !hash_string(fp))
        {
            return Err(Error::new(ErrorKind::InvalidData));
        }
        if !self.learning_enabled
            || event.learning_epoch != self.learning_epoch
            || self.recent_events.contains(&event.token)
        {
            return Ok(false);
        }
        if event.entries.iter().any(|(id, fp)| {
            id.starts_with("user:")
                && self.entries.get(id).is_none_or(|entry| {
                    !entry.enabled || fingerprint(&entry.text, &entry.pinyin) != *fp
                })
        }) {
            return Ok(false);
        }
        self.transaction(|next| {
            let entries: BTreeSet<_> = event.entries.iter().cloned().collect();
            for key in entries {
                if !next.preferences.contains_key(&key) && next.preferences.len() >= MAX_PREFERENCES
                {
                    return Err(Error::new(ErrorKind::Limit));
                }
                let count = next.preferences.entry(key).or_default();
                *count = count.saturating_add(1).min(MAX_COUNT);
            }
            if next.recent_events.len() == RECENT_EVENTS {
                next.recent_events.pop_front();
            }
            next.recent_events.push_back(event.token.clone());
            Ok(true)
        })
    }

    pub fn import_entries(&mut self, bytes: &[u8]) -> Result<usize> {
        let imported = Self::from_bytes(bytes)?;
        self.transaction(|next| {
            let mut added = 0;
            for (id, entry) in imported.entries {
                if let Some(existing) = next.entries.get(&id) {
                    if existing == &entry {
                        continue;
                    }
                    return Err(Error::new(ErrorKind::Conflict));
                }
                if next
                    .entries
                    .values()
                    .any(|e| e.text == entry.text && e.pinyin == entry.pinyin)
                {
                    return Err(Error::new(ErrorKind::Conflict));
                }
                if next.entries.len() == MAX_ENTRIES {
                    return Err(Error::new(ErrorKind::Limit));
                }
                next.entries.insert(id, entry);
                added += 1;
            }
            next.next_id = next.next_id.max(imported.next_id);
            if added > 0 {
                next.invalidate_selection_epoch()?;
            }
            Ok(added)
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut text = format!(
            "# cloudleaf-user-v2\ngeneration\t{}\nnext_id\t{}\nlearning\t{}\nlearning_epoch\t{}\npurge\t{}\nentries\t{}\nid\trevision\tenabled\ttext\tpinyin\n",
            self.generation,
            self.next_id,
            u8::from(self.learning_enabled),
            self.learning_epoch,
            u8::from(self.purge_history),
            self.entries.len()
        );
        for entry in self.entries.values() {
            text.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                entry.id,
                entry.revision,
                u8::from(entry.enabled),
                entry.text,
                entry.pinyin
            ));
        }
        text.push_str(&format!(
            "preferences\t{}\nid\tfingerprint\tcount\n",
            self.preferences.len()
        ));
        for ((id, fp), count) in &self.preferences {
            text.push_str(&format!("{id}\t{fp}\t{count}\n"));
        }
        let checksum = hex(digest(&SHA256, text.as_bytes()).as_ref());
        text.push_str(&format!("sha256\t{checksum}\n"));
        text.into_bytes()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::new(ErrorKind::Limit));
        }
        let text = std::str::from_utf8(bytes).map_err(|_| Error::new(ErrorKind::InvalidData))?;
        if text.contains('\r') || text.starts_with('\u{feff}') {
            return Err(Error::new(ErrorKind::InvalidData));
        }
        let version = if text.starts_with("# cloudleaf-user-v2\n") {
            2
        } else if text.starts_with("# cloudleaf-user-v1\n") {
            1
        } else if text.starts_with("# cloudleaf-user-v") {
            return Err(Error::new(ErrorKind::UnsupportedVersion));
        } else {
            return Err(Error::new(ErrorKind::InvalidData));
        };
        let (body, check) = text
            .rsplit_once("sha256\t")
            .ok_or(Error::new(ErrorKind::InvalidData))?;
        let check = check
            .strip_suffix('\n')
            .ok_or(Error::new(ErrorKind::InvalidData))?;
        if !body.ends_with('\n')
            || !hash_string(check)
            || hex(digest(&SHA256, body.as_bytes()).as_ref()) != check
        {
            return Err(Error::new(ErrorKind::InvalidData));
        }
        let mut rows = body.lines();
        rows.next();
        let mut result = Self::new();
        result.generation = number(field(rows.next(), "generation")?)?;
        result.next_id = number(field(rows.next(), "next_id")?)?;
        if result.next_id == 0 {
            return Err(Error::new(ErrorKind::InvalidData));
        }
        if version == 2 {
            result.learning_enabled = boolean(field(rows.next(), "learning")?)?;
            result.learning_epoch = number(field(rows.next(), "learning_epoch")?)?;
            result.purge_history = boolean(field(rows.next(), "purge")?)?;
            if result.learning_epoch == 0 {
                return Err(Error::new(ErrorKind::InvalidData));
            }
        }
        let count = number(field(rows.next(), "entries")?)?;
        if count > MAX_ENTRIES as u64 {
            return Err(Error::new(ErrorKind::Limit));
        }
        let columns = if version == 2 {
            "id\trevision\tenabled\ttext\tpinyin"
        } else {
            "id\ttext\tpinyin"
        };
        if rows.next() != Some(columns) {
            return Err(Error::new(ErrorKind::InvalidData));
        }
        let mut pairs = BTreeSet::new();
        for _ in 0..count {
            let fields: Vec<_> = rows
                .next()
                .ok_or(Error::new(ErrorKind::InvalidData))?
                .split('\t')
                .collect();
            if fields.len() != if version == 2 { 5 } else { 3 } {
                return Err(Error::new(ErrorKind::InvalidData));
            }
            let id_value = user_id(fields[0])?;
            if id_value >= result.next_id {
                return Err(Error::new(ErrorKind::InvalidData));
            }
            let (revision, enabled, text, pinyin) = if version == 2 {
                (
                    number(fields[1])?,
                    boolean(fields[2])?,
                    fields[3],
                    fields[4],
                )
            } else {
                (1, true, fields[1], fields[2])
            };
            validate_pair(text, pinyin)?;
            if revision == 0
                || !pairs.insert((text, pinyin))
                || result.entries.contains_key(fields[0])
            {
                return Err(Error::new(ErrorKind::Conflict));
            }
            result.entries.insert(
                fields[0].into(),
                UserEntry {
                    id: fields[0].into(),
                    revision,
                    enabled,
                    text: text.into(),
                    pinyin: pinyin.into(),
                },
            );
        }
        if version == 2 {
            let count = number(field(rows.next(), "preferences")?)?;
            if count > MAX_PREFERENCES as u64 {
                return Err(Error::new(ErrorKind::Limit));
            }
            if rows.next() != Some("id\tfingerprint\tcount") {
                return Err(Error::new(ErrorKind::InvalidData));
            }
            for _ in 0..count {
                let fields: Vec<_> = rows
                    .next()
                    .ok_or(Error::new(ErrorKind::InvalidData))?
                    .split('\t')
                    .collect();
                if fields.len() != 3 || !identifier(fields[0]) || !hash_string(fields[1]) {
                    return Err(Error::new(ErrorKind::InvalidData));
                }
                let count = number(fields[2])?;
                if count == 0
                    || count > u64::from(MAX_COUNT)
                    || result
                        .preferences
                        .insert((fields[0].into(), fields[1].into()), count as u32)
                        .is_some()
                {
                    return Err(Error::new(ErrorKind::InvalidData));
                }
            }
        }
        if rows.next().is_some() {
            return Err(Error::new(ErrorKind::InvalidData));
        }
        Ok(result)
    }

    fn transaction<T>(&mut self, action: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let mut next = self.clone();
        let output = action(&mut next)?;
        if next.to_bytes().len() > MAX_BYTES {
            return Err(Error::new(ErrorKind::Limit));
        }
        *self = next;
        Ok(output)
    }

    fn invalidate_selection_epoch(&mut self) -> Result<()> {
        self.learning_epoch = self
            .learning_epoch
            .checked_add(1)
            .ok_or(Error::new(ErrorKind::Limit))?;
        self.recent_events.clear();
        Ok(())
    }
}

pub fn fingerprint(text: &str, pinyin: &str) -> String {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes.extend_from_slice(pinyin.as_bytes());
    hex(digest(&SHA256, &bytes).as_ref())
}

pub fn choice_units(count: u32) -> u8 {
    if count < 2 {
        0
    } else {
        count.ilog2().min(8) as u8
    }
}

fn validate_pair(text: &str, pinyin: &str) -> Result<()> {
    let count = text.chars().count();
    if count == 0 || count > 16 || !text.chars().all(|c| matches!(u32::from(c), 0x3007 | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff | 0x20000..=0x323af))
        || pinyin.len() > 80 || pinyin.split(' ').count() != count
        || pinyin.bytes().filter(|b| *b != b' ').count() > 64
        || pinyin.split(' ').any(|s| !s.bytes().all(|b| b.is_ascii_lowercase()) || !cloudleaf_pinyin::is_syllable(s)) {
        return Err(Error::new(ErrorKind::InvalidData));
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn hash_string(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn user_id(value: &str) -> Result<u64> {
    let value = value
        .strip_prefix("user:")
        .ok_or(Error::new(ErrorKind::InvalidData))?;
    let number = number(value)?;
    if number == 0 {
        return Err(Error::new(ErrorKind::InvalidData));
    }
    Ok(number)
}
fn number(value: &str) -> Result<u64> {
    let result: u64 = value
        .parse()
        .map_err(|_| Error::new(ErrorKind::InvalidData))?;
    if result.to_string() != value {
        return Err(Error::new(ErrorKind::InvalidData));
    }
    Ok(result)
}
fn boolean(value: &str) -> Result<bool> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(Error::new(ErrorKind::InvalidData)),
    }
}
fn field<'a>(row: Option<&'a str>, key: &str) -> Result<&'a str> {
    let (name, value) = row
        .and_then(|r| r.split_once('\t'))
        .ok_or(Error::new(ErrorKind::InvalidData))?;
    if name != key {
        return Err(Error::new(ErrorKind::InvalidData));
    }
    Ok(value)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
