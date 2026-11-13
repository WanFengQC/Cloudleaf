use crate::{Error, ErrorKind, MAX_BYTES, Result, UserDictionary};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const MAX_MANAGED_FILES: usize = 64;

pub struct Store {
    root: PathBuf,
    _guard: File,
}
impl Drop for Store {
    fn drop(&mut self) {
        // Unix重复文件描述可能延长flock生命周期；明确交还所有权后再关闭本句柄。
        let _ = self._guard.unlock();
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveFault {
    None,
    BeforeWrite,
    PartialWrite,
    StorageFull,
    AfterSync,
    AfterPublish,
}

impl Store {
    pub fn open(root: PathBuf) -> Result<Self> {
        check_path(&root)?;
        for parent in root.ancestors() {
            if parent.join(".git").exists() {
                return Err(Error::new(ErrorKind::InvalidData));
            }
        }
        fs::create_dir_all(&root).map_err(Error::io)?;
        let guard = root.join(".writer.guard");
        check_path(&guard)?;
        let guard = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(guard)
            .map_err(Error::io)?;
        guard.try_lock().map_err(|_| Error::new(ErrorKind::Busy))?;
        Ok(Self {
            root,
            _guard: guard,
        })
    }

    pub fn load(&mut self) -> Result<UserDictionary> {
        let (generations, pending) = self.files()?;
        let Some((&generation, path)) = generations.last_key_value() else {
            return if pending.is_empty() {
                Ok(UserDictionary::new())
            } else {
                Err(Error::new(ErrorKind::RecoveryRequired))
            };
        };
        let mut dictionary = read_generation(path, generation)
            .map_err(|_| Error::new(ErrorKind::RecoveryRequired))?;
        if dictionary.purge_history {
            self.prune(generation, true)?;
            dictionary.purge_history = false;
        }
        Ok(dictionary)
    }

    pub fn recovery_candidates(&self) -> Result<Vec<u64>> {
        let (generations, _) = self.files()?;
        Ok(generations
            .iter()
            .rev()
            .filter_map(|(generation, path)| {
                read_generation(path, *generation).ok().map(|_| *generation)
            })
            .collect())
    }

    pub fn pending_count(&self) -> Result<usize> {
        self.files().map(|(_, pending)| pending.len())
    }

    pub fn latest_generation(&self) -> Result<u64> {
        self.files().map(|(generations, _)| {
            generations
                .last_key_value()
                .map_or(0, |(generation, _)| *generation)
        })
    }

    pub fn save(
        &mut self,
        dictionary: &UserDictionary,
        expected_generation: u64,
    ) -> Result<UserDictionary> {
        self.save_with_fault(dictionary, expected_generation, SaveFault::None)
    }

    /// 同一真实写入流程的显式研发故障点；正常宿主只调用save。
    #[doc(hidden)]
    pub fn save_with_fault(
        &mut self,
        dictionary: &UserDictionary,
        expected_generation: u64,
        fault: SaveFault,
    ) -> Result<UserDictionary> {
        let (generations, _) = self.files()?;
        let generation = generations
            .last_key_value()
            .map_or(0, |(generation, _)| *generation);
        if let Some(path) = generations.get(&generation) {
            read_generation(path, generation)
                .map_err(|_| Error::new(ErrorKind::RecoveryRequired))?;
        }
        if expected_generation != generation || dictionary.generation != expected_generation {
            return Err(Error::new(ErrorKind::Conflict));
        }
        self.publish(dictionary, generation, fault, false)
    }

    pub fn recover(&mut self, generation: u64) -> Result<UserDictionary> {
        let (generations, _) = self.files()?;
        let path = generations
            .get(&generation)
            .ok_or(Error::new(ErrorKind::Conflict))?;
        let dictionary = read_generation(path, generation)?;
        let highest = generations
            .last_key_value()
            .map_or(0, |(generation, _)| *generation);
        if generation < highest && read_generation(&generations[&highest], highest).is_ok() {
            return Err(Error::new(ErrorKind::Conflict));
        }
        self.publish(&dictionary, highest, SaveFault::None, true)
    }

    pub fn clear_all(&mut self, expected_generation: u64) -> Result<UserDictionary> {
        if self.latest_generation()? != expected_generation {
            return Err(Error::new(ErrorKind::Conflict));
        }
        let old = match self.load() {
            Ok(value) => Some(value),
            Err(error) if error.kind == ErrorKind::RecoveryRequired => None,
            Err(error) => return Err(error),
        };
        let mut empty = UserDictionary::new();
        empty.generation = expected_generation;
        if let Some(old) = old {
            empty.next_id = old.next_id;
            empty.learning_epoch = old
                .learning_epoch
                .checked_add(1)
                .ok_or(Error::new(ErrorKind::Limit))?;
        }
        // 显式清空全部在损坏档案上建立新档案；普通删除仍不重用ID。
        empty.purge_history = true;
        self.publish(&empty, expected_generation, SaveFault::None, false)
    }

    pub fn export(&self, dictionary: &UserDictionary, path: &Path) -> Result<()> {
        check_path(path)?;
        if path.starts_with(&self.root) {
            return Err(Error::new(ErrorKind::InvalidData));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(Error::io)?;
        file.write_all(&dictionary.to_bytes())
            .and_then(|_| file.sync_all())
            .map_err(Error::io)
    }

    pub fn import(&self, path: &Path, dictionary: &UserDictionary) -> Result<UserDictionary> {
        let bytes = read_bounded(path)?;
        let mut result = dictionary.clone();
        result.import_entries(&bytes)?;
        Ok(result)
    }

    fn publish(
        &self,
        dictionary: &UserDictionary,
        last_generation: u64,
        fault: SaveFault,
        preserve_originals: bool,
    ) -> Result<UserDictionary> {
        let generation = last_generation
            .checked_add(1)
            .ok_or(Error::new(ErrorKind::Limit))?;
        let mut next = dictionary.clone();
        next.generation = generation;
        let bytes = next.to_bytes();
        let mut created = None;
        for slot in 0..32 {
            let path = self
                .root
                .join(format!(".pending-{generation:020}-{slot:02}.clu"));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    created = Some((path, file));
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(Error::io(error)),
            }
        }
        let (pending, mut file) = created.ok_or(Error::new(ErrorKind::Limit))?;
        if fault == SaveFault::BeforeWrite {
            return Err(Error::new(ErrorKind::Io));
        }
        if matches!(fault, SaveFault::PartialWrite | SaveFault::StorageFull) {
            file.write_all(&bytes[..bytes.len() / 2])
                .map_err(Error::io)?;
            return Err(Error::io(std::io::Error::from(
                if fault == SaveFault::StorageFull {
                    std::io::ErrorKind::StorageFull
                } else {
                    std::io::ErrorKind::Interrupted
                },
            )));
        }
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(Error::io)?;
        drop(file);
        if fault == SaveFault::AfterSync {
            return Err(Error::new(ErrorKind::Io));
        }
        let loaded = UserDictionary::from_bytes(&read_bounded(&pending)?)?;
        if loaded != next {
            return Err(Error::new(ErrorKind::InvalidData));
        }
        fs::rename(&pending, self.root.join(generation_name(generation))).map_err(Error::io)?;
        if fault == SaveFault::AfterPublish {
            return Err(Error::new(ErrorKind::Unknown));
        }
        self.sync_directory()
            .map_err(|_| Error::new(ErrorKind::Unknown))?;
        if !preserve_originals {
            self.prune(generation, next.purge_history)
                .map_err(|_| Error::new(ErrorKind::Unknown))?;
        }
        next.purge_history = false;
        Ok(next)
    }

    fn prune(&self, keep: u64, all_history: bool) -> Result<()> {
        let (generations, pending) = self.files()?;
        let previous = generations
            .range(..keep)
            .next_back()
            .map(|(generation, _)| *generation);
        for (generation, path) in generations {
            if generation != keep && (all_history || Some(generation) != previous) {
                fs::remove_file(path).map_err(Error::io)?;
            }
        }
        for path in pending {
            fs::remove_file(path).map_err(Error::io)?;
        }
        self.sync_directory()
    }

    fn files(&self) -> Result<(BTreeMap<u64, PathBuf>, Vec<PathBuf>)> {
        let mut generations = BTreeMap::new();
        let mut pending = Vec::new();
        for (index, entry) in fs::read_dir(&self.root).map_err(Error::io)?.enumerate() {
            if index >= MAX_MANAGED_FILES {
                return Err(Error::new(ErrorKind::Limit));
            }
            let entry = entry.map_err(Error::io)?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(generation) = parse_generation(&name) {
                check_path(&entry.path())?;
                generations.insert(generation, entry.path());
            } else if is_pending(&name) {
                check_path(&entry.path())?;
                pending.push(entry.path());
            }
        }
        Ok((generations, pending))
    }

    fn sync_directory(&self) -> Result<()> {
        #[cfg(unix)]
        File::open(&self.root)
            .and_then(|file| file.sync_all())
            .map_err(Error::io)?;
        // Windows这里只保证真实进程中断的逻辑恢复，不宣称目录刷盘或物理断电保证。
        Ok(())
    }
}

fn read_generation(path: &Path, generation: u64) -> Result<UserDictionary> {
    let dictionary = UserDictionary::from_bytes(&read_bounded(path)?)?;
    if dictionary.generation != generation {
        return Err(Error::new(ErrorKind::InvalidData));
    }
    Ok(dictionary)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    check_path(path)?;
    let file = File::open(path).map_err(Error::io)?;
    if !file.metadata().map_err(Error::io)?.is_file() {
        return Err(Error::new(ErrorKind::InvalidData));
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(Error::io)?;
    if bytes.len() > MAX_BYTES {
        return Err(Error::new(ErrorKind::Limit));
    }
    Ok(bytes)
}

fn check_path(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(Error::new(ErrorKind::InvalidData));
    }
    for part in path.ancestors() {
        let metadata = match fs::symlink_metadata(part) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(Error::io(error)),
        };
        if metadata.file_type().is_symlink() {
            return Err(Error::new(ErrorKind::InvalidData));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(Error::new(ErrorKind::InvalidData));
            }
        }
    }
    Ok(())
}

fn generation_name(generation: u64) -> String {
    format!("generation-{generation:020}.clu")
}
fn parse_generation(name: &str) -> Option<u64> {
    let value = name.strip_prefix("generation-")?.strip_suffix(".clu")?;
    if value.len() != 20 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let generation = value.parse().ok()?;
    (generation != 0).then_some(generation)
}
fn is_pending(name: &str) -> bool {
    let Some(value) = name
        .strip_prefix(".pending-")
        .and_then(|v| v.strip_suffix(".clu"))
    else {
        return false;
    };
    let Some((generation, slot)) = value.split_once('-') else {
        return false;
    };
    generation.len() == 20
        && generation.bytes().all(|b| b.is_ascii_digit())
        && slot.len() == 2
        && slot.parse::<u8>().is_ok_and(|slot| slot < 32)
}

#[cfg(test)]
mod lock_lifetime_tests {
    use super::*;
    #[test]
    fn dropping_store_unlocks_even_when_a_duplicate_descriptor_survives() {
        let root = std::env::temp_dir().join(format!(
            "Cloudleaf-lock-drop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = Store::open(root.clone()).unwrap();
        let duplicate = store._guard.try_clone().unwrap();
        drop(store);
        let reopened =
            Store::open(root.clone()).expect("重复描述不能延长已经结束的Store写者生命周期");
        drop(reopened);
        drop(duplicate);
        fs::remove_dir_all(root).unwrap();
    }
}
