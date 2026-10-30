use crate::platform::CredentialStore;
use crate::settings::{ProviderChoice, Settings, SettingsRepository};
use cloudleaf_translation::*;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(crate) enum SetupCommand {
    Load(Option<PathBuf>),
    Prepare(Settings, bool),
    SaveKey(Settings, Secret),
    DeleteKey(Settings),
}

pub(crate) struct Prepared {
    pub settings: Settings,
    pub service: Option<TranslationService>,
    pub identity: Option<ProviderIdentity>,
    pub authorization: Option<ProviderAuthorization>,
    pub capabilities: Option<ProviderCapabilities>,
    pub notice: String,
    pub error: Option<String>,
}

pub(crate) enum SetupEvent {
    Prepared(Box<Prepared>),
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
                let repository = SettingsRepository::new(directory);
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
                        SetupCommand::Prepare(settings, persist) => {
                            if settings.validate().is_err() {
                                SetupEvent::Prepared(Box::new(unavailable(
                                    settings,
                                    "设置无效，请检查端点、模型和超时",
                                )))
                            } else if persist && repository.save(&settings).is_err() {
                                SetupEvent::Prepared(Box::new(unavailable(
                                    settings,
                                    "设置保存失败，未启用新提供者",
                                )))
                            } else {
                                SetupEvent::Prepared(Box::new(prepare(
                                    settings,
                                    store.clone(),
                                    "设置已应用".into(),
                                )))
                            }
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
        notice: reason.into(),
        error: Some(reason.into()),
    }
}

fn prepare(settings: Settings, store: Arc<dyn CredentialStore>, notice: String) -> Prepared {
    let built: Result<_, TranslationError> = (|| {
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
            ProviderChoice::OfflineModel => return Err(TranslationError::ModelNotFound),
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
                offline_only: !settings.network_enabled,
                ..ServiceLimits::default()
            },
        )?;
        Ok((service, identity, authorization, capabilities))
    })();
    match built {
        Ok((service, identity, authorization, capabilities)) => Prepared {
            settings,
            service: Some(service),
            identity: Some(identity),
            authorization,
            capabilities: Some(capabilities),
            notice,
            error: None,
        },
        Err(error) => {
            let reason = match error {
                TranslationError::ModelNotFound => {
                    "离线模型未安装，本程序尚无模型推理后端".to_owned()
                }
                TranslationError::ProviderUnavailable => {
                    "本地研究词典未配置；请指定导出的 catalog.json 并应用设置".to_owned()
                }
                _ => format!("提供者不可用：{error}。请检查设置与词典文件"),
            };
            unavailable(settings, &reason)
        }
    }
}
