use crate::center::{UserAction, UserDictionaryView, user_action};
use crate::platform::CredentialStore;
use crate::settings::{ProviderChoice, Settings, SettingsRepository};
use cloudleaf_settings::{
    InputPreferences, InputPreferencesRepository, InputSettingsError, InputSettingsStatus,
    LoadedInputPreferences,
};
use cloudleaf_translation::*;
use cloudleaf_translation_integrity::{Glossary, MAX_GLOSSARY_BYTES};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(crate) enum SetupCommand {
    Load(Option<PathBuf>),
    Apply(Settings),
    ReadInput,
    SaveInput(InputPreferences),
    UserDictionary(PathBuf, Option<u64>, UserAction),
    SaveKey(Settings, Secret),
    DeleteKey(Settings),
}

pub(crate) struct Prepared {
    pub settings: Settings,
    pub service: Option<TranslationService>,
    pub identity: Option<ProviderIdentity>,
    pub authorization: Option<ProviderAuthorization>,
    pub capabilities: Option<ProviderCapabilities>,
    pub worker: Option<WorkerObserver>,
    pub glossary: Glossary,
    pub notice: String,
    pub error: Option<String>,
}

pub(crate) enum SetupEvent {
    Prepared(Box<Prepared>),
    Saved(Box<Prepared>),
    SaveRejected(String),
    Input(Result<LoadedInputPreferences, InputSettingsError>, bool),
    User(Result<UserDictionaryView, cloudleaf_user::Error>),
    Key(Result<&'static str, crate::platform::PlatformError>),
}

pub(crate) struct SetupHandle {
    pub sender: Option<mpsc::SyncSender<SetupCommand>>,
    pub receiver: mpsc::Receiver<SetupEvent>,
    pub thread: JoinHandle<()>,
}

impl SetupHandle {
    pub fn start(directory: PathBuf, store: Arc<dyn CredentialStore>) -> std::io::Result<Self> {
        let (sender, commands) = mpsc::sync_channel(8);
        let (events, receiver) = mpsc::sync_channel(8);
        let thread = thread::Builder::new()
            .name("cloudleaf-panel-settings".into())
            .spawn(move || {
                let repository = SettingsRepository::new(directory.clone());
                let input = InputPreferencesRepository::new(directory);
                while let Ok(command) = commands.recv() {
                    let event = match command {
                        SetupCommand::Load(dictionary) => {
                            let loaded = repository.load();
                            let (mut settings, notice) = match loaded {
                                Ok(value) => (value.settings, value.notice.unwrap_or_default()),
                                Err(_) => (
                                    Settings::default(),
                                    "设置读取失败，已使用离线默认值；原文件未修改".into(),
                                ),
                            };
                            if dictionary.is_some() {
                                settings.dictionary_path = dictionary;
                            }
                            SetupEvent::Prepared(Box::new(prepare(settings, store.clone(), notice)))
                        }
                        SetupCommand::Apply(settings) => {
                            let prepared = prepare(settings, store.clone(), "设置已应用".into());
                            match repository.save(&prepared.settings) {
                                Ok(()) => SetupEvent::Saved(Box::new(prepared)),
                                Err(error) => SetupEvent::SaveRejected(format!(
                                    "设置保存失败，保留当前提供者：{error}"
                                )),
                            }
                        }
                        SetupCommand::ReadInput => SetupEvent::Input(input.load(), false),
                        SetupCommand::SaveInput(settings) => SetupEvent::Input(
                            input
                                .save(&settings)
                                .map(|settings| LoadedInputPreferences {
                                    settings,
                                    status: InputSettingsStatus::Current,
                                }),
                            true,
                        ),
                        SetupCommand::UserDictionary(directory, generation, action) => {
                            SetupEvent::User(user_action(&directory, generation, action))
                        }
                        SetupCommand::SaveKey(settings, secret) => SetupEvent::Key(
                            store
                                .set(&settings.credential_reference(), secret)
                                .map(|()| "密钥已保存到当前用户的凭据管理器"),
                        ),
                        SetupCommand::DeleteKey(settings) => SetupEvent::Key(
                            store.delete(&settings.credential_reference()).map(|found| {
                                if found {
                                    "已删除该端点的密钥"
                                } else {
                                    "该端点没有已保存密钥"
                                }
                            }),
                        ),
                    };
                    if events.send(event).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            receiver,
            thread,
        })
    }
}

fn unavailable(settings: Settings, reason: &str) -> Prepared {
    Prepared {
        settings,
        service: None,
        identity: None,
        authorization: None,
        capabilities: None,
        worker: None,
        glossary: Glossary::default(),
        notice: reason.into(),
        error: Some(reason.into()),
    }
}

fn prepare(settings: Settings, store: Arc<dyn CredentialStore>, notice: String) -> Prepared {
    let glossary = match (&settings.provider, &settings.offline_glossary) {
        (ProviderChoice::OfflineModel, Some(path)) => match load_glossary(path) {
            Some(glossary) => glossary,
            None => return unavailable(settings, "离线术语表无法读取或格式无效，请检查设置"),
        },
        _ => Glossary::default(),
    };
    let built: Result<_, TranslationError> = (|| {
        let mut worker = None;
        let (provider, authorization): (Arc<dyn TranslationProvider>, _) = match settings.provider {
            ProviderChoice::Dictionary => {
                let path = settings
                    .dictionary_path
                    .as_ref()
                    .ok_or(TranslationError::ProviderUnavailable)?;
                (
                    Arc::new(LocalDictionaryProvider::load(
                        "local-dictionary",
                        path,
                        ReadPolicy::Research,
                        None,
                    )?),
                    None,
                )
            }
            ProviderChoice::Online => {
                let secrets: Arc<dyn SecretStore> = store;
                let provider = OpenAiCompatibleProvider::new(
                    OpenAiCompatibleConfig {
                        endpoint: settings.endpoint.clone(),
                        model: settings.model.clone(),
                        credential_reference: settings.credential_reference(),
                        enabled: settings.network_enabled,
                        config_version: settings.revision,
                        allow_loopback_http: settings.allow_loopback_http,
                        connect_timeout: Duration::from_secs(settings.connect_timeout_secs),
                        ..OpenAiCompatibleConfig::default()
                    },
                    secrets,
                )?;
                let authorization = provider.authorization();
                (Arc::new(provider), Some(authorization))
            }
            ProviderChoice::OfflineModel => {
                let catalog = settings
                    .offline_catalog
                    .as_ref()
                    .ok_or(TranslationError::ModelNotFound)?;
                let expected_sha256 = worker_catalog_sha256(catalog)?;
                let config = settings
                    .worker_config(catalog.clone(), expected_sha256)
                    .map_err(TranslationError::OfflineWorker)?;
                let provider = WorkerProvider::new(config);
                worker = Some(provider.observer());
                (Arc::new(provider), None)
            }
        };
        let identity = provider.identity();
        let capabilities = provider.capabilities();
        let mut registry = ProviderRegistry::new();
        registry.register(
            provider,
            TranslationProviderConfig {
                allow_network: settings.network_enabled,
                allow_cache: settings.cache_enabled,
                timeout: Duration::from_secs(settings.total_timeout_secs),
                config_version: settings.revision,
                read_policy: if settings.provider == ProviderChoice::Dictionary {
                    ReadPolicy::Research
                } else {
                    ReadPolicy::Production
                },
                ..TranslationProviderConfig::default()
            },
        )?;
        let service = TranslationService::new(
            registry,
            ServiceLimits {
                workers: if settings.provider == ProviderChoice::OfflineModel {
                    1
                } else {
                    2
                },
                offline_only: !settings.network_enabled,
                ..ServiceLimits::default()
            },
        )?;
        Ok((service, identity, authorization, capabilities, worker))
    })();
    match built {
        Ok((service, identity, authorization, capabilities, worker)) => Prepared {
            settings: settings.clone(),
            service: Some(service),
            identity: Some(identity),
            authorization,
            capabilities: Some(capabilities),
            worker,
            glossary,
            notice: if settings.provider == ProviderChoice::OfflineModel {
                format!("已配置离线模型，提交翻译时启动工作进程。{notice}")
            } else {
                notice
            },
            error: None,
        },
        Err(error) => {
            let reason = match error {
                TranslationError::ModelNotFound => {
                    "NotInstalled · 离线模型未安装、未配置或缺文件；请选择已导入并校验的模型 catalog"
                        .to_owned()
                }
                TranslationError::ProviderUnavailable => {
                    "本地研究词典未配置；请指定导出的 catalog.json 并应用设置".to_owned()
                }
                _ if settings.provider == ProviderChoice::OfflineModel => {
                    format!("离线模型不可用：{error}；请用模型检查工具核对目录")
                }
                _ => format!("提供者不可用：{error}。请检查设置与词典文件"),
            };
            unavailable(settings, &reason)
        }
    }
}

fn load_glossary(path: &Path) -> Option<Glossary> {
    let mut text = String::new();
    File::open(path)
        .ok()?
        .take(MAX_GLOSSARY_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .ok()?;
    Glossary::parse_tsv(&text).ok()
}
