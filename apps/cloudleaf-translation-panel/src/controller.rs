use crate::platform::{CredentialStore, PlatformError};
use crate::settings::{DirectionChoice, Settings};
use crate::setup::{Prepared, SetupCommand, SetupEvent, SetupHandle};
use cloudleaf_translation::*;
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
    context: TranslationContext,
    focused: bool,
    panel: TranslationPanel,
    service: Option<TranslationService>,
    identity: Option<ProviderIdentity>,
    authorization: Option<ProviderAuthorization>,
    capabilities: Option<ProviderCapabilities>,
    active: Option<(TranslationTaskId, TranslationContext, u64, Instant)>,
    setup: SetupHandle,
    preparing: bool,
    pending: Option<Settings>,
    closing: Option<Instant>,
}

impl PanelController {
    pub fn new(
        directory: PathBuf,
        dictionary: Option<PathBuf>,
        store: Arc<dyn CredentialStore>,
    ) -> std::io::Result<Self> {
        let setup = SetupHandle::start(directory, store)?;
        setup
            .sender
            .as_ref()
            .expect("新建通道")
            .try_send(SetupCommand::Load(dictionary))
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
            notice: "正在后台载入设置和研究词典…".into(),
            error: None,
            elapsed: None,
            authorized: false,
            dirty: false,
            settings_serial: 0,
            context,
            focused: true,
            panel: TranslationPanel::new(context),
            service: None,
            identity: None,
            authorization: None,
            capabilities: None,
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
        settings.revision = self
            .settings
            .revision
            .checked_add(1)
            .ok_or(TranslationError::InvalidConfig)?;
        self.invalidate();
        self.context.epoch += 1;
        self.panel.focus(self.context);
        if let Some(service) = &mut self.service {
            service.begin_shutdown();
        }
        self.pending = Some(settings);
        self.status = ViewStatus::Preparing;
        self.notice = "正在等待旧任务退出并应用设置…".into();
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), TranslationError> {
        if self.preparing || self.pending.is_some() || self.dirty || self.closing.is_some() {
            return Err(TranslationError::Busy);
        }
        if !self.focused {
            return Err(TranslationError::StaleResult);
        }
        self.cancel_active();
        self.panel.cancel();
        self.panel.focus(self.context);
        self.error = None;
        self.elapsed = None;
        let identity = self
            .identity
            .clone()
            .ok_or(TranslationError::ProviderUnavailable)?;
        let context = self.context;
        let request = TranslationRequest {
            text: self.text.clone(),
            direction: direction(self.direction),
            kind: self.kind,
            context,
            send_authorized_to: if self.authorized {
                self.authorization.clone()
            } else {
                None
            },
            use_cache: self.settings.cache_enabled,
        };
        let service = self
            .service
            .as_mut()
            .ok_or(TranslationError::ProviderUnavailable)?;
        let id = service.submit(&identity.id, request.clone())?;
        self.panel.start(id, &request, identity)?;
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
        self.panel.result()
    }

    pub fn copy_text(&self) -> Result<String, TranslationError> {
        let result = self.panel.result().ok_or(TranslationError::StaleResult)?;
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
        self.error = Some(format!("{error}"));
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
                Ok(SetupEvent::Key(outcome)) => match outcome {
                    Ok(notice) => self.notice = notice.into(),
                    Err(error) => self.platform_error(error),
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.closing.is_none() {
                        self.error = Some("设置线程已退出，请重新启动面板".into());
                        self.preparing = false;
                    }
                    break;
                }
            }
        }
        if self.pending.is_some()
            && self
                .service
                .as_ref()
                .is_none_or(TranslationService::workers_finished)
        {
            if let Some(service) = self.service.take() {
                let _ = service.shutdown();
            }
            let settings = self.pending.take().expect("已检查pending");
            match self.send(SetupCommand::Prepare(settings, true)) {
                Ok(()) => self.preparing = true,
                Err(error) => self.record_error(error),
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
                        self.elapsed = Some(started.elapsed());
                        self.error = snapshot.error.map(|value| value.to_string());
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
        self.service
            .as_ref()
            .is_none_or(TranslationService::workers_finished)
            && self.setup.thread.is_finished()
    }

    pub fn close_deadline_reached(&self) -> bool {
        self.closing
            .is_some_and(|start| start.elapsed() >= Duration::from_secs(2))
    }

    pub fn platform_error(&mut self, error: PlatformError) {
        self.error = Some(format!("系统操作失败：{error:?}，内容仍保留"));
    }

    fn adopt(&mut self, mut value: Prepared) {
        if self.closing.is_some() {
            if let Some(service) = &mut value.service {
                service.begin_shutdown();
            }
            self.service = value.service;
            self.preparing = false;
            return;
        }
        self.settings = value.settings;
        self.direction = self.settings.direction;
        self.service = value.service;
        self.identity = value.identity;
        self.authorization = value.authorization;
        self.capabilities = value.capabilities;
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
        self.cancel_active();
        self.panel.cancel();
        self.context.input_revision += 1;
        if self.focused {
            self.panel.focus(self.context);
        }
        self.authorized = false;
        self.elapsed = None;
        self.error = None;
        if !self.preparing && self.closing.is_none() {
            self.status = ViewStatus::Idle;
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
