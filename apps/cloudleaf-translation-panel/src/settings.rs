use cloudleaf_settings::{read_bounded, write_atomic};
use cloudleaf_translation::{OpenAiCompatibleConfig, OpenAiCompatibleProvider, SessionSecretStore};
use cloudleaf_translation_ipc::{IpcError, ResourceThreads, WorkerConfig, WorkerScheduling};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const MAX_SETTINGS_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerThreads {
    #[default]
    One,
    Two,
    Four,
    Auto,
}

impl WorkerThreads {
    fn is_default(value: &Self) -> bool {
        *value == Self::default()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerPriority {
    #[default]
    Normal,
    BelowNormal,
}

impl WorkerPriority {
    fn is_default(value: &Self) -> bool {
        *value == Self::default()
    }
}

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
    pub offline_catalog: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline_glossary: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "WorkerThreads::is_default")]
    pub worker_threads: WorkerThreads,
    #[serde(default, skip_serializing_if = "WorkerPriority::is_default")]
    pub worker_priority: WorkerPriority,
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
            schema_version: 3,
            revision: 1,
            provider: ProviderChoice::Dictionary,
            dictionary_path: None,
            offline_catalog: None,
            offline_glossary: None,
            worker_threads: WorkerThreads::One,
            worker_priority: WorkerPriority::Normal,
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
    pub(crate) fn worker_config(
        &self,
        catalog: PathBuf,
        digest: String,
    ) -> Result<WorkerConfig, IpcError> {
        let config = match self.worker_threads {
            WorkerThreads::One => WorkerConfig::sibling(catalog, digest, ResourceThreads::One),
            WorkerThreads::Two => WorkerConfig::sibling(catalog, digest, ResourceThreads::Two),
            WorkerThreads::Four => WorkerConfig::sibling(catalog, digest, ResourceThreads::Four),
            WorkerThreads::Auto => WorkerConfig::sibling_auto(catalog, digest),
        }?;
        config.with_scheduling(match self.worker_priority {
            WorkerPriority::Normal => WorkerScheduling::Normal,
            WorkerPriority::BelowNormal => WorkerScheduling::BelowNormal,
        })
    }

    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.schema_version != 3 {
            return Err(SettingsError::UnsupportedVersion);
        }
        if self.revision == 0
            || !(1..=300).contains(&self.connect_timeout_secs)
            || !(1..=300).contains(&self.total_timeout_secs)
            || self.connect_timeout_secs > self.total_timeout_secs
            || self.endpoint.len() > 2048
            || self.model.len() > 256
            || self.model.chars().any(char::is_control)
            || [
                &self.dictionary_path,
                &self.offline_catalog,
                &self.offline_glossary,
            ]
            .into_iter()
            .flatten()
            .any(|path| {
                let value = path.to_string_lossy();
                value.is_empty() || value.contains('\0') || value.len() > 4096
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
        let primary = match read_bounded(&main, MAX_SETTINGS_BYTES) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(SettingsError::Io),
        };
        if let Some(bytes) = &primary
            && let Ok((settings, migrated)) = parse(bytes)
        {
            return Ok(prepare_loaded(settings, migrated, None));
        }
        match read_bounded(
            &self.directory.join("settings.json.bak"),
            MAX_SETTINGS_BYTES,
        ) {
            Ok(bytes) => {
                if let Ok((settings, migrated)) = parse(&bytes) {
                    return Ok(prepare_loaded(
                        settings,
                        migrated,
                        Some("主设置缺失或无效，已读取有效备份；原文件未改动"),
                    ));
                }
            }
            Err(error) if primary.is_none() && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LoadedSettings {
                    settings: Settings::default(),
                    notice: None,
                });
            }
            Err(_) => (),
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
        match read_bounded(&main, MAX_SETTINGS_BYTES) {
            Ok(previous) => match parse(&previous) {
                Ok(_) => write_atomic(&self.directory.join("settings.json.bak"), &previous)
                    .map_err(|_| SettingsError::Io)?,
                Err(SettingsError::UnsupportedVersion) => {
                    return Err(SettingsError::UnsupportedVersion);
                }
                Err(_) => (),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(SettingsError::Io),
        }
        write_atomic(&main, &bytes).map_err(|_| SettingsError::Io)
    }
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
    let migrated = version == 1 || version == 2;
    let settings: Settings = if migrated {
        let mut defaults =
            serde_json::to_value(Settings::default()).map_err(|_| SettingsError::InvalidData)?;
        let object = defaults.as_object_mut().ok_or(SettingsError::InvalidData)?;
        for (key, value) in value.as_object().ok_or(SettingsError::InvalidData)? {
            object.insert(key.clone(), value.clone());
        }
        object.insert("schema_version".into(), 3.into());
        object.insert("network_enabled".into(), false.into());
        object.insert("allow_loopback_http".into(), false.into());
        serde_json::from_value(defaults).map_err(|_| SettingsError::InvalidData)?
    } else if version == 3 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round27_worker_configuration_binds_catalog_identity_and_thread_preference() {
        let legacy = serde_json::to_vec(&Settings::default()).unwrap();
        let (legacy, migrated) = parse(&legacy).unwrap();
        assert!(!migrated);
        assert_eq!(legacy.worker_threads, WorkerThreads::One);
        assert_eq!(legacy.worker_priority, WorkerPriority::Normal);
        let settings = Settings {
            worker_threads: WorkerThreads::Four,
            worker_priority: WorkerPriority::BelowNormal,
            ..legacy
        };
        let (restored, migrated) = parse(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(restored, settings);
        assert!(!migrated);
        let catalog = PathBuf::from("models/catalog.json");
        let sha = "a".repeat(64);
        if cfg!(windows) {
            let child = restored
                .worker_config(catalog.clone(), sha.clone())
                .unwrap();
            assert_eq!(child.compute_threads(), ResourceThreads::Four);
            assert_eq!(child.catalog_path(), catalog);
            assert_eq!(child.catalog_sha256(), sha);
        } else {
            assert!(matches!(
                restored.worker_config(catalog, sha),
                Err(IpcError::InvalidConfiguration)
            ));
        }
    }
}
