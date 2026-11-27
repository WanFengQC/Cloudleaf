use crate::center::{InputSettingsView, UserAction, UserSettingsView};
use crate::definitions::{DefinitionDirection, DefinitionRequest, DefinitionTask, DefinitionView};
use crate::platform::{CredentialStore, PlatformError};
use crate::resources::{ResourceIdentity, ResourceRequest, ResourceSlot};
use crate::settings::{DirectionChoice, ProviderChoice, Settings};
use crate::setup::{Prepared, SetupCommand, SetupEvent, SetupHandle};
use cloudleaf_settings::{
    DictionaryArtifact, InputPreferences, LanguageResourceConfig, ResourceAction, ResourceReceipt,
    ResourceSlot as DictionarySlot, RuntimeSettingsReceipt,
};
use cloudleaf_translation::*;
use cloudleaf_translation_integrity::{self as integrity, Glossary, IntegrityReport};
use cloudleaf_translation_ipc::IpcError;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewStatus {
    Preparing,
    Idle,
    Queued,
    Running,
    Completed,
    Cancelled,
    TimedOut,
    Failed,
    ProviderUnavailable,
    Closing,
}

pub struct PanelController {
    pub settings: Settings,
    pub text: String,
    pub direction: DirectionChoice,
    pub kind: TranslationKind,
    pub status: ViewStatus,
    pub notice: String,
    pub error: Option<String>,
    pub elapsed: Option<Duration>,
    pub authorized: bool,
    pub dirty: bool,
    pub settings_serial: u64,
    pub input_settings: InputSettingsView,
    pub user_settings: UserSettingsView,
    pub definitions: DefinitionView,
    pub runtime_settings: Vec<RuntimeSettingsReceipt>,
    pub runtime_notice: &'static str,
    runtime_pending: bool,
    pub resources: [Option<ResourceIdentity>; 2],
    pub resource_notice: [&'static str; 2],
    pub dictionary_config: Option<LanguageResourceConfig>,
    pub dictionary_hosts: Vec<ResourceReceipt>,
    pub dictionary_notice: String,
    pub dictionary_saving: bool,
    next_definition: u64,
    context: TranslationContext,
    focused: bool,
    panel: TranslationPanel,
    service: Option<TranslationService>,
    identity: Option<ProviderIdentity>,
    authorization: Option<ProviderAuthorization>,
    capabilities: Option<ProviderCapabilities>,
    worker: Option<WorkerObserver>,
    worker_shutdown_error: Option<TranslationError>,
    glossary: Glossary,
    integrity: Option<IntegrityReport>,
    connection_check: bool,
    active: Option<(TranslationTaskId, TranslationContext, u64, Instant)>,
    setup: SetupHandle,
    preparing: bool,
    pending: Option<Box<Prepared>>,
    closing: Option<Instant>,
}

impl PanelController {
    pub fn new(
        directory: PathBuf,
        dictionary: Option<PathBuf>,
        store: Arc<dyn CredentialStore>,
    ) -> std::io::Result<Self> {
        let setup = SetupHandle::start(directory, store)?;
        let sender = setup.sender.as_ref().expect("新建通道");
        sender
            .try_send(SetupCommand::Load(dictionary))
            .and_then(|()| sender.try_send(SetupCommand::ReadInput))
            .map_err(|_| std::io::Error::other("settings worker unavailable"))?;
        let settings = Settings::default();
        let context = TranslationContext {
            session: u64::from(std::process::id()),
            epoch: 1,
            input_revision: 1,
        };
        Ok(Self {
            direction: settings.direction,
            settings,
            text: String::new(),
            kind: TranslationKind::Word,
            status: ViewStatus::Preparing,
            notice: "正在后台载入设置和本地资源…".into(),
            error: None,
            elapsed: None,
            authorized: false,
            dirty: false,
            settings_serial: 0,
            input_settings: InputSettingsView {
                working: true,
                ..InputSettingsView::default()
            },
            user_settings: UserSettingsView::default(),
            definitions: DefinitionView::default(),
            runtime_settings: Vec::new(),
            runtime_notice: "",
            runtime_pending: false,
            resources: [None, None],
            resource_notice: [""; 2],
            dictionary_config: None,
            dictionary_hosts: Vec::new(),
            dictionary_notice: String::new(),
            dictionary_saving: false,
            next_definition: 1,
            context,
            focused: true,
            panel: TranslationPanel::new(context),
            service: None,
            identity: None,
            authorization: None,
            capabilities: None,
            worker: None,
            worker_shutdown_error: None,
            glossary: Glossary::default(),
            integrity: None,
            connection_check: false,
            active: None,
            setup,
            preparing: true,
            pending: None,
            closing: None,
        })
    }

    pub fn set_text(&mut self, text: String) {
        if self.text != text {
            self.invalidate();
            self.text = text;
        }
    }

    pub fn clear_definitions(&mut self) {
        self.definitions = DefinitionView::default();
    }

    pub fn lookup_definition(
        &mut self,
        path: PathBuf,
        sha256: String,
        direction: DefinitionDirection,
    ) -> Result<(), TranslationError> {
        self.clear_definitions();
        let text = self.text.trim();
        if text.is_empty()
            || text.len() > 4096
            || text.contains('\0')
            || !path.is_absolute()
            || sha256.len() != 64
            || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(TranslationError::InvalidConfig);
        }
        if !self.focused || self.closing.is_some() {
            return Err(TranslationError::ProviderUnavailable);
        }
        let task = DefinitionTask {
            id: self.next_definition,
            input_revision: self.context.input_revision,
        };
        self.next_definition = self
            .next_definition
            .checked_add(1)
            .ok_or(TranslationError::Busy)?;
        self.setup
            .sender
            .as_ref()
            .ok_or(TranslationError::ProviderUnavailable)?
            .try_send(SetupCommand::Define(DefinitionRequest {
                task,
                path,
                sha256: sha256.to_ascii_lowercase(),
                text: text.to_owned(),
                direction,
            }))
            .map_err(|_| TranslationError::Busy)?;
        self.definitions.task = Some(task);
        self.definitions.pending = true;
        self.definitions.notice = "正在核验索引并查询释义…".into();
        Ok(())
    }

    pub fn set_direction(&mut self, direction: DirectionChoice) {
        if self.direction != direction {
            self.invalidate();
            self.direction = direction;
        }
    }

    pub fn set_kind(&mut self, kind: TranslationKind) {
        if self.kind != kind {
            self.invalidate();
            self.kind = kind;
        }
    }

    pub fn mark_settings_dirty(&mut self) {
        self.invalidate();
        self.dirty = true;
        self.notice = "设置已修改，请应用后再翻译".into();
    }

    pub fn supports_kind(&self, kind: TranslationKind) -> bool {
        self.capabilities
            .as_ref()
            .is_some_and(|value| value.kinds.contains(&kind))
    }

    pub fn supports_auto(&self) -> bool {
        self.capabilities
            .as_ref()
            .is_some_and(|value| value.auto_source)
    }

    pub fn available(&self) -> bool {
        self.service.is_some()
            && self.worker_shutdown_error.is_none()
            && self
                .capabilities
                .as_ref()
                .is_some_and(|value| value.available)
            && !self.preparing
            && self.pending.is_none()
    }

    pub fn apply(&mut self, mut settings: Settings) -> Result<(), TranslationError> {
        if self.preparing || self.pending.is_some() || self.closing.is_some() {
            return Err(TranslationError::Busy);
        }
        if settings.validate().is_err() {
            return Err(TranslationError::InvalidConfig);
        }
        if let Some(error) = self.worker_shutdown_error {
            return Err(error);
        }
        settings.revision = self
            .settings
            .revision
            .checked_add(1)
            .ok_or(TranslationError::InvalidConfig)?;
        self.send(SetupCommand::Apply(settings))?;
        self.invalidate();
        self.context.epoch += 1;
        self.panel.focus(self.context);
        self.preparing = true;
        self.status = ViewStatus::Preparing;
        self.notice = "正在后台核验并保存设置…".into();
        Ok(())
    }

    pub fn reload_input_settings(&mut self) -> Result<(), TranslationError> {
        self.request_input(SetupCommand::ReadInput)
    }

    pub fn refresh_runtime_settings(&mut self) {
        if !self.runtime_pending
            && self.closing.is_none()
            && self.send(SetupCommand::ReadRuntime).is_ok()
        {
            self.runtime_pending = true;
        }
    }

    pub fn inspect_resource(&mut self, slot: ResourceSlot, path: PathBuf, sha256: String) {
        let index = slot.index();
        if self.closing.is_some() || self.resource_notice[index] == "正在后台核验" {
            return;
        }
        if !path.is_absolute()
            || sha256.len() != 64
            || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            self.resource_notice[index] = "需要绝对路径和完整 SHA-256";
            return;
        }
        let request = ResourceRequest {
            slot,
            path,
            sha256: sha256.to_ascii_lowercase(),
        };
        self.resource_notice[index] = if self.send(SetupCommand::InspectResource(request)).is_ok() {
            "正在后台核验"
        } else {
            "暂时无法开始检查"
        };
    }

    pub fn select_dictionary(&mut self, slot: ResourceSlot, path: PathBuf, sha256: String) {
        let artifact = match DictionaryArtifact::new(path, sha256) {
            Ok(artifact) => artifact,
            Err(error) => {
                self.dictionary_notice = error.to_string();
                return;
            }
        };
        self.manage_dictionary(ResourceAction::Select(
            match slot {
                ResourceSlot::Base => DictionarySlot::Base,
                ResourceSlot::Supplement => DictionarySlot::Supplement,
            },
            artifact,
        ));
    }

    pub fn manage_dictionary(&mut self, action: ResourceAction) {
        if self.dictionary_saving || self.closing.is_some() {
            return;
        }
        let Some(config) = &self.dictionary_config else {
            self.dictionary_notice = "当前词典配置尚未读取，请稍后重试".into();
            self.refresh_runtime_settings();
            return;
        };
        if self
            .send(SetupCommand::SaveDictionaryRuntime(config.revision, action))
            .is_ok()
        {
            self.dictionary_saving = true;
            self.dictionary_notice = "正在校验已登记身份并保存词典配置".into();
        } else {
            self.dictionary_notice = "未能开始词典配置保存".into();
        }
    }

    pub fn save_input_settings(
        &mut self,
        settings: InputPreferences,
    ) -> Result<(), TranslationError> {
        self.request_input(SetupCommand::SaveInput(settings))
    }

    fn request_input(&mut self, command: SetupCommand) -> Result<(), TranslationError> {
        if self.input_settings.working || self.closing.is_some() {
            return Err(TranslationError::Busy);
        }
        self.send(command)?;
        self.input_settings.working = true;
        Ok(())
    }

    pub fn manage_user_dictionary(
        &mut self,
        directory: PathBuf,
        action: UserAction,
    ) -> Result<(), TranslationError> {
        if self.user_settings.working || self.closing.is_some() {
            return Err(TranslationError::Busy);
        }
        if self.user_settings.needs_refresh && !matches!(&action, UserAction::Inspect) {
            return Err(TranslationError::StaleResult);
        }
        if !directory.is_absolute() {
            return Err(TranslationError::InvalidConfig);
        }
        if !matches!(&action, UserAction::Inspect)
            && self.user_settings.directory.as_ref() != Some(&directory)
        {
            self.user_settings.notice = "请先读取当前目录并核对词库".into();
            return Err(TranslationError::StaleResult);
        }
        self.send(SetupCommand::UserDictionary(
            directory.clone(),
            self.user_settings.dictionary.generation,
            action,
        ))?;
        self.user_settings.pending_directory = Some(directory);
        self.user_settings.working = true;
        Ok(())
    }

    pub fn provider_identity(&self) -> Option<&ProviderIdentity> {
        self.identity.as_ref()
    }

    pub fn start(&mut self) -> Result<(), TranslationError> {
        self.start_request(false)
    }

    /// 只发送固定探测短句，仍要求应用后的端点与单次授权。
    pub fn test_connection(&mut self) -> Result<(), TranslationError> {
        if self.settings.provider != ProviderChoice::Online || !self.settings.network_enabled {
            return Err(TranslationError::ProviderUnavailable);
        }
        if !self.authorized {
            return Err(TranslationError::OnlineNotAuthorized);
        }
        self.start_request(true)
    }

    fn start_request(&mut self, connection_check: bool) -> Result<(), TranslationError> {
        if self.preparing || self.pending.is_some() || self.dirty || self.closing.is_some() {
            return Err(TranslationError::Busy);
        }
        if !self.focused {
            return Err(TranslationError::StaleResult);
        }
        if let Some(error) = self.worker_shutdown_error {
            return Err(error);
        }
        self.cancel_active();
        self.panel.cancel();
        self.context.input_revision += 1;
        self.panel.focus(self.context);
        self.error = None;
        self.elapsed = None;
        self.integrity = None;
        let identity = self
            .identity
            .clone()
            .ok_or(TranslationError::ProviderUnavailable)?;
        let context = self.context;
        let (text, requested_direction, kind) = if connection_check {
            (
                "Cloudleaf connectivity check.".to_owned(),
                DirectionChoice::EnglishChinese,
                TranslationKind::Sentence,
            )
        } else {
            (self.text.clone(), self.direction, self.kind)
        };
        let request = TranslationRequest {
            text,
            direction: direction(requested_direction),
            kind,
            context,
            send_authorized_to: if std::mem::take(&mut self.authorized) {
                self.authorization.clone()
            } else {
                None
            },
            use_cache: !connection_check && self.settings.cache_enabled,
        };
        let service = self
            .service
            .as_mut()
            .ok_or(TranslationError::ProviderUnavailable)?;
        let id = service.submit(&identity.id, request.clone())?;
        self.panel.start(id, &request, identity)?;
        self.connection_check = connection_check;
        self.active = Some((id, context, self.settings.revision, Instant::now()));
        self.status = ViewStatus::Queued;
        self.tick();
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.invalidate();
        self.status = ViewStatus::Cancelled;
        self.notice = "已取消，原文保留".into();
    }

    pub fn focus(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.invalidate();
        self.context.epoch += 1;
        self.focused = focused;
        if focused {
            self.panel.focus(self.context);
        } else {
            self.panel.focus_lost();
        }
        self.status = ViewStatus::Cancelled;
    }

    pub fn result(&self) -> Option<&TranslationResult> {
        self.panel.result().filter(|_| !self.connection_check)
    }

    pub fn worker_snapshot(&self) -> Option<WorkerSnapshot> {
        self.worker.as_ref().and_then(WorkerObserver::snapshot)
    }

    pub fn integrity_report(&self) -> Option<&IntegrityReport> {
        self.integrity.as_ref()
    }

    pub fn copy_text(&self) -> Result<String, TranslationError> {
        let result = self.result().ok_or(TranslationError::StaleResult)?;
        Ok(self
            .panel
            .copy(result.task_id, self.context, RESULT_VERSION)?
            .text)
    }

    pub fn record_error(&mut self, error: TranslationError) {
        self.status = if error == TranslationError::ProviderUnavailable {
            ViewStatus::ProviderUnavailable
        } else {
            ViewStatus::Failed
        };
        self.error = Some(translation_message(error));
    }

    pub fn save_key(&mut self, settings: Settings, secret: Secret) -> Result<(), TranslationError> {
        if settings.endpoint.is_empty() || settings.validate().is_err() {
            return Err(TranslationError::InvalidConfig);
        }
        self.invalidate();
        if let Some(service) = &mut self.service {
            service.clear_cache();
        }
        self.send(SetupCommand::SaveKey(settings, secret))
    }

    pub fn delete_key(&mut self, settings: Settings) -> Result<(), TranslationError> {
        if settings.endpoint.is_empty() || settings.validate().is_err() {
            return Err(TranslationError::InvalidConfig);
        }
        self.invalidate();
        if let Some(service) = &mut self.service {
            service.clear_cache();
        }
        self.send(SetupCommand::DeleteKey(settings))
    }

    pub fn tick(&mut self) {
        for _ in 0..8 {
            match self.setup.receiver.try_recv() {
                Ok(SetupEvent::Prepared(value)) => self.adopt(*value),
                Ok(SetupEvent::Saved(value)) if self.closing.is_none() => {
                    if let Some(service) = &mut self.service {
                        service.begin_shutdown();
                    }
                    self.pending = Some(value);
                    self.notice = "设置已保存，等待旧工作进程退出…".into();
                }
                Ok(SetupEvent::Saved(_)) => (),
                Ok(SetupEvent::SaveRejected(reason)) => {
                    self.preparing = false;
                    self.dirty = false;
                    self.settings_serial += 1;
                    self.status = if self.available() {
                        ViewStatus::Idle
                    } else {
                        ViewStatus::ProviderUnavailable
                    };
                    self.notice = reason;
                }
                Ok(SetupEvent::Input(result, saved)) => self.input_settings.complete(result, saved),
                Ok(SetupEvent::Runtime(result)) => {
                    self.runtime_pending = false;
                    self.runtime_settings.clear();
                    self.runtime_notice = match result.settings {
                        Ok(receipts) if receipts.is_empty() => "未收到存活宿主回执",
                        Ok(receipts) => {
                            self.runtime_settings = receipts;
                            ""
                        }
                        Err(_) => "无法读取运行时设置回执",
                    };
                    match result.dictionary {
                        Ok(config) => self.dictionary_config = Some(config),
                        Err(error) => {
                            self.dictionary_config = None;
                            self.dictionary_notice = error;
                        }
                    }
                    match result.hosts {
                        Ok(hosts) => {
                            let changed = hosts.iter().any(|host| {
                                self.dictionary_hosts.iter().any(|old| {
                                    old.host_id == host.host_id && old.active != host.active
                                })
                            });
                            self.dictionary_hosts = hosts;
                            if changed {
                                self.invalidate();
                            }
                        }
                        Err(error) => {
                            self.dictionary_hosts.clear();
                            self.dictionary_notice = error;
                        }
                    }
                }
                Ok(SetupEvent::DictionaryRuntimeSaved(result)) => {
                    self.dictionary_saving = false;
                    match result {
                        Ok(config) => {
                            self.dictionary_notice =
                                format!("修订 {} 已保存，等待宿主核验和应用", config.revision);
                            self.dictionary_config = Some(config);
                        }
                        Err(error) => self.dictionary_notice = error,
                    }
                    self.refresh_runtime_settings();
                }
                Ok(SetupEvent::Resource(slot, result)) => {
                    let i = slot.index();
                    self.resource_notice[i] = match result {
                        Ok(identity) => {
                            self.resources[i] = Some(identity);
                            "已核验格式与内容 SHA-256"
                        }
                        Err(reason) => reason,
                    };
                }
                Ok(SetupEvent::Defined(task, result)) => {
                    if self.definitions.task == Some(task)
                        && task.input_revision == self.context.input_revision
                        && self.closing.is_none()
                        && self.focused
                    {
                        self.definitions.pending = false;
                        match result {
                            Ok(found) => {
                                self.definitions.notice = if found.total == 0 {
                                    "没有对应释义"
                                } else {
                                    "释义已就绪"
                                }
                                .into();
                                self.definitions.contents = found.text;
                            }
                            Err(err) => self.definitions.notice = translation_message(err),
                        }
                    }
                }
                Ok(SetupEvent::User(result)) => self.user_settings.complete(result),
                Ok(SetupEvent::Key(outcome)) => match outcome {
                    Ok(notice) => self.notice = notice.into(),
                    Err(error) => self.platform_error(error),
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.closing.is_none() {
                        self.error = Some("设置线程已退出，请重新启动面板".into());
                        self.preparing = false;
                        if self.definitions.pending {
                            self.definitions.pending = false;
                            self.definitions.notice = "释义查询服务不可用".into();
                        }
                    }
                    break;
                }
            }
        }
        if (self.pending.is_some() || self.closing.is_some())
            && self.worker_shutdown_error.is_none()
            && self
                .service
                .as_ref()
                .is_none_or(TranslationService::workers_finished)
        {
            let retired = self
                .service
                .as_mut()
                .map(TranslationService::finish_shutdown);
            if let Some(Err(error)) = retired {
                if self.worker.is_some() {
                    self.worker_shutdown_error = Some(error);
                }
                if self.closing.is_some() {
                    self.error = Some(translation_message(error));
                } else {
                    self.record_error(error);
                }
            }
            if self.worker_shutdown_error.is_none() {
                self.service = None;
            }
            if let Some(value) = self.pending.take() {
                if retired.is_some_and(|result| result.is_err()) {
                    self.preparing = false;
                    self.notice = "设置已保存，旧提供者关闭失败，新设置未生效".into();
                } else {
                    self.adopt(*value);
                }
            }
        }
        if let Some((id, context, revision, started)) = self.active {
            let snapshot = self
                .service
                .as_mut()
                .ok_or(TranslationError::ProviderUnavailable)
                .and_then(|service| service.poll(id));
            match snapshot {
                Ok(snapshot)
                    if revision == self.settings.revision
                        && self.panel.update(id, context, snapshot.clone()).is_ok() =>
                {
                    self.status = match snapshot.state {
                        TaskState::Queued => ViewStatus::Queued,
                        TaskState::Running => ViewStatus::Running,
                        TaskState::Succeeded => ViewStatus::Completed,
                        TaskState::Cancelled => ViewStatus::Cancelled,
                        TaskState::Expired => {
                            if snapshot.error == Some(TranslationError::Timeout) {
                                ViewStatus::TimedOut
                            } else {
                                ViewStatus::Cancelled
                            }
                        }
                        TaskState::Failed => {
                            if snapshot.error == Some(TranslationError::ProviderUnavailable) {
                                ViewStatus::ProviderUnavailable
                            } else {
                                ViewStatus::Failed
                            }
                        }
                    };
                    if snapshot.state.is_terminal() {
                        if self.connection_check {
                            self.notice = if snapshot.state == TaskState::Succeeded {
                                "已收到 API 测试响应。原文保留"
                            } else {
                                "API 测试未成功。原文保留"
                            }
                            .into();
                        }
                        if snapshot.state == TaskState::Succeeded && self.worker.is_some() {
                            self.update_integrity();
                        }
                        self.elapsed = Some(started.elapsed());
                        self.error = snapshot.error.map(translation_message);
                        self.active = None;
                        if let Some(service) = &mut self.service {
                            let _ = service.forget(id);
                        }
                    }
                }
                Ok(_) => {
                    self.cancel_active();
                }
                Err(error) => {
                    self.cancel_active();
                    self.record_error(error);
                }
            }
        }
    }

    pub fn begin_close(&mut self) {
        if self.closing.is_some() {
            return;
        }
        self.invalidate();
        self.pending = None;
        if let Some(service) = &mut self.service {
            service.begin_shutdown();
        }
        self.setup.sender = None;
        self.closing = Some(Instant::now());
        self.status = ViewStatus::Closing;
    }

    pub fn close_ready(&self) -> bool {
        self.service.is_none()
            && self.worker_shutdown_error.is_none()
            && self.setup.thread.is_finished()
    }

    pub fn close_deadline_reached(&self) -> bool {
        let limit = if self.worker.is_some() {
            Duration::from_secs(4)
        } else {
            Duration::from_secs(2)
        };
        self.closing.is_some_and(|start| start.elapsed() >= limit)
    }

    pub fn platform_error(&mut self, error: PlatformError) {
        self.error = Some(format!("系统操作失败：{error}。原文和译文仍保留"));
    }

    fn adopt(&mut self, mut value: Prepared) {
        if self.closing.is_some() {
            if let Some(service) = &mut value.service {
                service.begin_shutdown();
            }
            self.service = value.service;
            self.worker = value.worker;
            self.preparing = false;
            return;
        }
        self.settings = value.settings;
        self.direction = self.settings.direction;
        self.service = value.service;
        self.identity = value.identity;
        self.authorization = value.authorization;
        self.capabilities = value.capabilities;
        if !self.supports_auto() {
            self.direction = match self.direction {
                DirectionChoice::AutoEnglish => DirectionChoice::ChineseEnglish,
                DirectionChoice::AutoChinese => DirectionChoice::EnglishChinese,
                explicit => explicit,
            };
        }
        self.worker = value.worker;
        self.glossary = value.glossary;
        if !self.supports_kind(self.kind) {
            self.kind = TranslationKind::Word;
        }
        self.notice = value.notice;
        self.error = value.error;
        self.dirty = false;
        self.preparing = false;
        self.authorized = false;
        self.settings_serial += 1;
        self.status = if self.available() {
            ViewStatus::Idle
        } else {
            ViewStatus::ProviderUnavailable
        };
        if self.service.is_some() && !self.available() {
            self.notice = "联网未启用，当前保持离线；请明确启用后应用设置".into();
        }
    }

    fn invalidate(&mut self) {
        self.clear_definitions();
        self.cancel_active();
        self.panel.cancel();
        self.context.input_revision += 1;
        if self.focused {
            self.panel.focus(self.context);
        }
        self.authorized = false;
        self.connection_check = false;
        self.elapsed = None;
        self.error = None;
        self.integrity = None;
        if !self.preparing && self.closing.is_none() {
            self.status = if self.available() {
                ViewStatus::Idle
            } else {
                ViewStatus::ProviderUnavailable
            };
        }
    }

    fn cancel_active(&mut self) {
        if let Some((id, _, _, _)) = self.active.take()
            && let Some(service) = &mut self.service
        {
            let _ = service.cancel(id);
            let _ = service.forget(id);
        }
    }

    fn update_integrity(&mut self) {
        let Some(result) = self.panel.result() else {
            return;
        };
        let direction = match result.direction.source {
            Language::Chinese => integrity::Direction::ChineseToEnglish,
            Language::English => integrity::Direction::EnglishToChinese,
            Language::Auto => return,
        };
        let original = self.panel.original();
        self.integrity = Some(integrity::check_translation(
            original,
            &result.output.translated_text,
            direction,
            &self.glossary,
        ));
    }

    fn send(&self, command: SetupCommand) -> Result<(), TranslationError> {
        self.setup
            .sender
            .as_ref()
            .ok_or(TranslationError::ProviderUnavailable)?
            .try_send(command)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => TranslationError::Busy,
                mpsc::TrySendError::Disconnected(_) => TranslationError::ProviderUnavailable,
            })
    }
}

impl Drop for PanelController {
    fn drop(&mut self) {
        self.begin_close();
    }
}

pub fn direction(value: DirectionChoice) -> TranslationDirection {
    match value {
        DirectionChoice::ChineseEnglish => TranslationDirection {
            source: Language::Chinese,
            target: Language::English,
        },
        DirectionChoice::EnglishChinese => TranslationDirection {
            source: Language::English,
            target: Language::Chinese,
        },
        DirectionChoice::AutoEnglish => TranslationDirection {
            source: Language::Auto,
            target: Language::English,
        },
        DirectionChoice::AutoChinese => TranslationDirection {
            source: Language::Auto,
            target: Language::Chinese,
        },
    }
}

fn translation_message(error: TranslationError) -> String {
    let message = match error {
        TranslationError::InputTooLong => "原文超过字节或当前提供者token限制，请缩短",
        TranslationError::InvalidInput => "请输入有效的非空文本",
        TranslationError::OnlineNotAuthorized => "本次尚未授权发送，请确认端点并勾选发送授权",
        TranslationError::MissingSecret => "该端点尚未保存API Key",
        TranslationError::Busy => "设置或任务正在处理，请稍候重试",
        TranslationError::ProviderUnavailable => "当前提供者不可用，请检查设置或安装状态",
        TranslationError::InvalidConfig => "设置无效，请检查端点、模型与超时",
        TranslationError::NoMatch => "研究词典没有完全匹配的词头或释义",
        TranslationError::AuthenticationFailed => "服务拒绝凭据，请检查该端点的API Key",
        TranslationError::Timeout => "翻译已超时，原文保留，可手动重试",
        TranslationError::NetworkUnavailable => "网络不可用，原文保留",
        TranslationError::ModelNotFound => "离线模型未安装或缺少目录文件",
        TranslationError::ModelLoadFailed => "离线模型初始化失败",
        TranslationError::OfflineCatalogMismatch => "离线模型目录已变化，请重新校验并应用设置",
        TranslationError::OfflineWorker(IpcError::ModelInvalid) => "离线模型校验失败",
        TranslationError::OfflineWorker(IpcError::WorkerStartFailed) => "无法启动离线翻译工作进程",
        TranslationError::OfflineWorker(IpcError::HandshakeTimeout) => "离线翻译工作进程握手超时",
        TranslationError::OfflineWorker(IpcError::InvalidWorkerPath) => {
            "离线翻译工作进程文件不存在或路径无效"
        }
        TranslationError::OfflineWorker(IpcError::InvalidProtocolVersion) => {
            "离线翻译工作进程协议版本不兼容"
        }
        TranslationError::OfflineWorker(IpcError::WorkerExited | IpcError::BrokenPipe) => {
            "离线翻译工作进程已退出，请按需重新请求"
        }
        TranslationError::OfflineWorker(IpcError::WorkerReapFailed) => {
            "无法确认旧工作进程退出，未启动重复实例"
        }
        TranslationError::OfflineWorker(_) => "离线翻译工作进程错误，原文保留",
        _ => "翻译未完成，原文保留；可检查设置或手动重试",
    };
    format!("{message} ({error})")
}

#[cfg(test)]
mod message_tests {
    use super::*;

    #[test]
    fn input_limit_message_covers_provider_tokens_and_bytes() {
        let message = translation_message(TranslationError::InputTooLong);
        assert!(message.contains("token"));
        assert!(message.contains("字节"));
    }
}
