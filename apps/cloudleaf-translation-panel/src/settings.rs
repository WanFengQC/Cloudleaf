use cloudleaf_translation::{OpenAiCompatibleConfig, OpenAiCompatibleProvider, SessionSecretStore};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_SETTINGS_BYTES: usize = 32 * 1024;
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderChoice {
    Dictionary,
    Online,
    OfflineModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectionChoice {
    ChineseEnglish,
    EnglishChinese,
    AutoEnglish,
    AutoChinese,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub revision: u64,
    pub provider: ProviderChoice,
    pub dictionary_path: Option<PathBuf>,
    pub endpoint: String,
    pub model: String,
    pub direction: DirectionChoice,
    pub connect_timeout_secs: u64,
    pub total_timeout_secs: u64,
    pub cache_enabled: bool,
    pub network_enabled: bool,
    pub allow_loopback_http: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 2,
            revision: 1,
            provider: ProviderChoice::Dictionary,
            dictionary_path: None,
            endpoint: String::new(),
            model: String::new(),
            direction: DirectionChoice::ChineseEnglish,
            connect_timeout_secs: 5,
            total_timeout_secs: 15,
            cache_enabled: false,
            network_enabled: false,
            allow_loopback_http: false,
        }
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("schema_version", &self.schema_version)
            .field("revision", &self.revision)
            .field("provider", &self.provider)
            .field("network_enabled", &self.network_enabled)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsError {
    InvalidData,
    InvalidConfig,
    UnsupportedVersion,
    TooLarge,
    Io,
}
impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidData => "设置文件格式无效",
            Self::InvalidConfig => "设置字段不符合约定",
            Self::UnsupportedVersion => "设置版本不受支持",
            Self::TooLarge => "设置文件超过32KiB限制",
            Self::Io => "无法读写设置文件",
        })
    }
}
impl std::error::Error for SettingsError {}

impl Settings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.schema_version != 2 {
            return Err(SettingsError::UnsupportedVersion);
        }
        if self.revision == 0
            || !(1..=300).contains(&self.connect_timeout_secs)
            || !(1..=300).contains(&self.total_timeout_secs)
            || self.connect_timeout_secs > self.total_timeout_secs
            || self.endpoint.len() > 2048
            || self.model.len() > 256
            || self.model.chars().any(char::is_control)
            || self.dictionary_path.as_ref().is_some_and(|path| {
                path.as_os_str().is_empty()
                    || path.to_string_lossy().contains('\0')
                    || path.to_string_lossy().len() > 4096
            })
        {
            return Err(SettingsError::InvalidConfig);
        }
        if self.endpoint.is_empty() {
            return if self.provider == ProviderChoice::Online {
                Err(SettingsError::InvalidConfig)
            } else {
                Ok(())
            };
        }
        OpenAiCompatibleProvider::new(
            OpenAiCompatibleConfig {
                endpoint: self.endpoint.clone(),
                model: self.model.clone(),
                credential_reference: self.credential_reference(),
                enabled: false,
                config_version: self.revision,
                allow_loopback_http: self.allow_loopback_http,
                connect_timeout: Duration::from_secs(self.connect_timeout_secs),
                ..OpenAiCompatibleConfig::default()
            },
            Arc::new(SessionSecretStore::new()),
        )
        .map_err(|_| SettingsError::InvalidConfig)?;
        Ok(())
    }

    pub fn credential_reference(&self) -> String {
        let suffix: String = digest(&SHA256, self.endpoint.as_bytes())
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("api-{suffix}")
    }
}

#[derive(Debug)]
pub struct LoadedSettings {
    pub settings: Settings,
    pub notice: Option<String>,
}

pub struct SettingsRepository {
    directory: PathBuf,
}
impl SettingsRepository {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn load(&self) -> Result<LoadedSettings, SettingsError> {
        let main = self.directory.join("settings.json");
        let primary = match read_file(&main) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(SettingsError::Io),
        };
        if let Some(bytes) = &primary {
            if let Ok((settings, migrated)) = parse(bytes) {
                return Ok(prepare_loaded(settings, migrated, None));
            }
        } else {
            return Ok(LoadedSettings {
                settings: Settings::default(),
                notice: None,
            });
        }
        if let Ok(bytes) = read_file(&self.directory.join("settings.json.bak"))
            && let Ok((settings, migrated)) = parse(&bytes)
        {
            return Ok(prepare_loaded(
                settings,
                migrated,
                Some("主设置无效，已读取有效备份；原文件未改动"),
            ));
        }
        Ok(LoadedSettings {
            settings: Settings::default(),
            notice: Some("设置和备份不可用，已使用离线默认值；原文件未改动".into()),
        })
    }

    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        settings.validate()?;
        let mut bytes =
            serde_json::to_vec_pretty(settings).map_err(|_| SettingsError::InvalidData)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_SETTINGS_BYTES {
            return Err(SettingsError::TooLarge);
        }
        fs::create_dir_all(&self.directory).map_err(|_| SettingsError::Io)?;
        let main = self.directory.join("settings.json");
        match read_file(&main) {
            Ok(previous) if parse(&previous).is_ok() => {
                write_replacement(&self.directory.join("settings.json.bak"), &previous)?
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(SettingsError::Io),
        }
        write_replacement(&main, &bytes)
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take((MAX_SETTINGS_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn parse(bytes: &[u8]) -> Result<(Settings, bool), SettingsError> {
    if bytes.len() > MAX_SETTINGS_BYTES {
        return Err(SettingsError::TooLarge);
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| SettingsError::InvalidData)?;
    let version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(SettingsError::InvalidData)?;
    let migrated = version == 1;
    let settings: Settings = if migrated {
        let mut defaults =
            serde_json::to_value(Settings::default()).map_err(|_| SettingsError::InvalidData)?;
        let object = defaults.as_object_mut().ok_or(SettingsError::InvalidData)?;
        for (key, value) in value.as_object().ok_or(SettingsError::InvalidData)? {
            object.insert(key.clone(), value.clone());
        }
        object.insert("schema_version".into(), 2.into());
        object.insert("network_enabled".into(), false.into());
        object.insert("allow_loopback_http".into(), false.into());
        serde_json::from_value(defaults).map_err(|_| SettingsError::InvalidData)?
    } else if version == 2 {
        serde_json::from_value(value).map_err(|_| SettingsError::InvalidData)?
    } else {
        return Err(SettingsError::UnsupportedVersion);
    };
    settings.validate()?;
    Ok((settings, migrated))
}

fn prepare_loaded(
    mut settings: Settings,
    migrated: bool,
    recovery: Option<&str>,
) -> LoadedSettings {
    let had_network = settings.network_enabled;
    settings.network_enabled = false;
    let mut notices = Vec::new();
    if let Some(recovery) = recovery {
        notices.push(recovery);
    }
    if migrated {
        notices.push("已迁移旧设置，联网和本机HTTP授权保持关闭");
    }
    if had_network {
        notices.push("启动已关闭联网，请明确重新启用并逐次授权");
    }
    LoadedSettings {
        settings,
        notice: (!notices.is_empty()).then(|| notices.join("；")),
    }
}

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_replacement(path: &Path, bytes: &[u8]) -> Result<(), SettingsError> {
    let parent = path.parent().ok_or(SettingsError::Io)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SettingsError::Io)?
        .as_nanos();
    let temporary_path = parent.join(format!(
        ".settings-{}-{timestamp}-{}.tmp",
        std::process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary_path)
        .map_err(|_| SettingsError::Io)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    // 仅清理本次create_new成功创建的文件，并先关闭Windows文件句柄。
    let temporary = TemporaryFile(temporary_path);
    written.map_err(|_| SettingsError::Io)?;
    replace(&temporary.0, path)?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| SettingsError::Io)?;
    Ok(())
}

#[cfg(not(windows))]
fn replace(from: &Path, to: &Path) -> Result<(), SettingsError> {
    fs::rename(from, to).map_err(|_| SettingsError::Io)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn replace(from: &Path, to: &Path) -> Result<(), SettingsError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if from[..from.len() - 1].contains(&0) || to[..to.len() - 1].contains(&0) {
        return Err(SettingsError::Io);
    }
    // 同目录替换并请求写穿；不能据此声称物理断电已经验收。
    if unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(SettingsError::Io);
    }
    Ok(())
}
