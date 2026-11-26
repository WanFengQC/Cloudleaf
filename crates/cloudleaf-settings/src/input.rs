use crate::{read_bounded, write_atomic};
use cloudleaf_input::{
    CorrectionConfig, DoublePinyinLayout, FuzzyDirection, FuzzyPair, FuzzyRule, InputConfig,
    MAX_CANDIDATES, PinyinScheme, SchemeConfig,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_BYTES: usize = 32 * 1024;
const PRIMARY: &str = "input-settings.json";
const BACKUP: &str = "input-settings.json.bak";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputScheme {
    #[default]
    Full,
    Abbreviated,
    Xiaohe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputFuzzyPair {
    ZhZ,
    ChC,
    ShS,
    NL,
    InIng,
    EnEng,
    AnAng,
}

impl InputFuzzyPair {
    pub const ALL: [Self; 7] = [
        Self::ZhZ,
        Self::ChC,
        Self::ShS,
        Self::NL,
        Self::InIng,
        Self::EnEng,
        Self::AnAng,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ZhZ => "zh / z",
            Self::ChC => "ch / c",
            Self::ShS => "sh / s",
            Self::NL => "n / l",
            Self::InIng => "in / ing",
            Self::EnEng => "en / eng",
            Self::AnAng => "an / ang",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputPreferences {
    pub schema_version: u32,
    pub revision: u64,
    pub scheme: InputScheme,
    pub fuzzy_pairs: Vec<InputFuzzyPair>,
    pub correction: bool,
    pub mixed_input: bool,
    pub page_size: usize,
    pub max_candidates: usize,
}

impl Default for InputPreferences {
    fn default() -> Self {
        let core = InputConfig::default();
        Self {
            schema_version: 1,
            revision: 1,
            scheme: InputScheme::Full,
            fuzzy_pairs: Vec::new(),
            correction: false,
            mixed_input: false,
            page_size: core.page_size,
            max_candidates: core.max_candidates,
        }
    }
}

impl InputPreferences {
    pub fn validate(&self) -> Result<(), InputSettingsError> {
        if self.schema_version != 1 {
            return Err(InputSettingsError::UnsupportedVersion);
        }
        if self.revision == 0
            || !(1..=9).contains(&self.page_size)
            || !(1..=MAX_CANDIDATES).contains(&self.max_candidates)
            || self.page_size > self.max_candidates
        {
            return Err(InputSettingsError::InvalidConfig);
        }
        self.scheme_config()
            .validate()
            .map_err(|_| InputSettingsError::InvalidConfig)
    }

    pub fn input_config(&self) -> InputConfig {
        InputConfig {
            page_size: self.page_size,
            max_candidates: self.max_candidates,
        }
    }

    pub fn scheme_config(&self) -> SchemeConfig {
        SchemeConfig {
            scheme: match self.scheme {
                InputScheme::Full => PinyinScheme::FullPinyin,
                InputScheme::Abbreviated => PinyinScheme::AbbreviatedPinyin,
                InputScheme::Xiaohe => PinyinScheme::DoublePinyin(DoublePinyinLayout::Xiaohe),
            },
            fuzzy: self
                .fuzzy_pairs
                .iter()
                .map(|pair| FuzzyRule {
                    pair: match pair {
                        InputFuzzyPair::ZhZ => FuzzyPair::ZhZ,
                        InputFuzzyPair::ChC => FuzzyPair::ChC,
                        InputFuzzyPair::ShS => FuzzyPair::ShS,
                        InputFuzzyPair::NL => FuzzyPair::NL,
                        InputFuzzyPair::InIng => FuzzyPair::InIng,
                        InputFuzzyPair::EnEng => FuzzyPair::EnEng,
                        InputFuzzyPair::AnAng => FuzzyPair::AnAng,
                    },
                    direction: FuzzyDirection::Both,
                })
                .collect(),
            correction: self.correction.then(CorrectionConfig::default),
            ..SchemeConfig::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSettingsStatus {
    Current,
    Backup,
    DefaultMissing,
    DefaultRecovered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedInputPreferences {
    pub settings: InputPreferences,
    pub status: InputSettingsStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSettingsError {
    InvalidConfig,
    UnsupportedVersion,
    StaleRevision,
    Io,
}

impl fmt::Display for InputSettingsError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::InvalidConfig => "输入设置无效，请检查方案、纠错及候选数量",
            Self::UnsupportedVersion => "输入设置版本不受支持",
            Self::StaleRevision => "输入设置已变更，请重读后再保存",
            Self::Io => "无法读写输入设置，保留当前配置",
        })
    }
}

impl std::error::Error for InputSettingsError {}

pub struct InputPreferencesRepository {
    directory: PathBuf,
}

impl InputPreferencesRepository {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn load(&self) -> Result<LoadedInputPreferences, InputSettingsError> {
        self.read().map(|(loaded, _)| loaded)
    }

    pub fn save(
        &self,
        settings: &InputPreferences,
    ) -> Result<InputPreferences, InputSettingsError> {
        settings.validate()?;
        let (current, previous) = self.read()?;
        if settings.revision != current.settings.revision {
            return Err(InputSettingsError::StaleRevision);
        }
        let mut saved = settings.clone();
        saved.revision = saved
            .revision
            .checked_add(1)
            .ok_or(InputSettingsError::StaleRevision)?;
        let mut bytes = serde_json::to_vec_pretty(&saved).map_err(|_| InputSettingsError::Io)?;
        bytes.push(b'\n');
        fs::create_dir_all(&self.directory).map_err(|_| InputSettingsError::Io)?;
        if let Some(previous) = previous {
            write_atomic(&self.directory.join(BACKUP), &previous)
                .map_err(|_| InputSettingsError::Io)?;
        }
        write_atomic(&self.directory.join(PRIMARY), &bytes).map_err(|_| InputSettingsError::Io)?;
        Ok(saved)
    }

    fn read(&self) -> Result<(LoadedInputPreferences, Option<Vec<u8>>), InputSettingsError> {
        let primary = read_optional(&self.directory.join(PRIMARY))?;
        if let Some(bytes) = primary.as_deref() {
            match parse(bytes) {
                Ok(settings) => {
                    return Ok((
                        LoadedInputPreferences {
                            settings,
                            status: InputSettingsStatus::Current,
                        },
                        primary,
                    ));
                }
                Err(InputSettingsError::UnsupportedVersion) => {
                    return Err(InputSettingsError::UnsupportedVersion);
                }
                Err(_) => (),
            }
        }
        let backup = read_optional(&self.directory.join(BACKUP))?;
        let backup_settings = match backup.as_deref().map(parse) {
            Some(Ok(settings)) => Some(settings),
            Some(Err(InputSettingsError::UnsupportedVersion)) => {
                return Err(InputSettingsError::UnsupportedVersion);
            }
            _ => None,
        };
        let loaded = if let Some(settings) = backup_settings {
            LoadedInputPreferences {
                settings,
                status: InputSettingsStatus::Backup,
            }
        } else {
            LoadedInputPreferences {
                settings: InputPreferences::default(),
                status: if primary.is_none() && backup.is_none() {
                    InputSettingsStatus::DefaultMissing
                } else {
                    InputSettingsStatus::DefaultRecovered
                },
            }
        };
        Ok((loaded, None))
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, InputSettingsError> {
    match read_bounded(path, MAX_BYTES) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(InputSettingsError::Io),
    }
}

fn parse(bytes: &[u8]) -> Result<InputPreferences, InputSettingsError> {
    if bytes.len() > MAX_BYTES {
        return Err(InputSettingsError::InvalidConfig);
    }
    let json: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| InputSettingsError::InvalidConfig)?;
    match json
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
    {
        Some(1) => (),
        Some(_) => return Err(InputSettingsError::UnsupportedVersion),
        None => return Err(InputSettingsError::InvalidConfig),
    }
    let settings: InputPreferences =
        serde_json::from_value(json).map_err(|_| InputSettingsError::InvalidConfig)?;
    settings.validate()?;
    Ok(settings)
}
