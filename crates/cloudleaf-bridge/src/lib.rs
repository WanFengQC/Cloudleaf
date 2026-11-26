//! 仅在显式操作时连接输入事务与后台翻译服务。

pub use cloudleaf_input::TranslationSourceIdentity as SourceIdentity;

use cloudleaf_input::{
    InputEngine, LanguageSnapshot, MAX_TRANSLATION_COMMIT_BYTES, SchemeConfig, SearchSnapshot,
};
use cloudleaf_translation::{
    Language, OutputKind, ProviderIdentity, ProviderRegistry, QualityStatus, RESULT_VERSION,
    ServiceLimits, TaskState, TranslationContext, TranslationDirection, TranslationError,
    TranslationKind, TranslationProvider, TranslationProviderConfig, TranslationRequest,
    TranslationResult, TranslationService, TranslationTaskId,
};
use cloudleaf_translation_integrity::{
    Direction as IntegrityDirection, Glossary, IntegrityReport, check_translation,
};
use cloudleaf_types::{
    CommitOutcome, CommitRequest, CoreError, ErrorCode, EventResult, InputEvent, InputMode,
    InputState, Key,
};
use std::{fmt, sync::Arc, time::Duration};

pub const TEXT_LIMIT: usize = MAX_TRANSLATION_COMMIT_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationState {
    Idle,
    Requested,
    Running,
    Ready,
    Failed,
    Cancelled,
    Stale,
    CommitPending,
    RetryReady,
    CommitUnknown,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    Translation(TranslationError),
    Input(CoreError),
    InvalidState,
    StaleConfirmation,
    StaleReceipt,
    OutcomeUnknown,
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Translation(error) => error.fmt(f),
            Self::Input(error) => error.fmt(f),
            value => write!(f, "{value:?}"),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<TranslationError> for BridgeError {
    fn from(error: TranslationError) -> Self {
        Self::Translation(error)
    }
}

impl From<CoreError> for BridgeError {
    fn from(error: CoreError) -> Self {
        Self::Input(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitIdentity {
    pub context: TranslationContext,
    pub task: TranslationTaskId,
    pub commit_id: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PendingCommit {
    pub identity: CommitIdentity,
    pub request: CommitRequest,
}

impl fmt::Debug for PendingCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingCommit")
            .field("identity", &self.identity)
            .field("text_bytes", &self.request.text.len())
            .finish()
    }
}

enum Phase {
    Requested,
    Running,
    Ready(TranslationResult),
    Failed(TranslationError),
    Cancelled,
    Stale,
    CommitPending { id: u64, result: TranslationResult },
    RetryReady { id: u64, result: TranslationResult },
    CommitUnknown { id: u64, result: TranslationResult },
    Completed,
}

struct Active {
    task: TranslationTaskId,
    context: TranslationContext,
    source: SourceIdentity,
    direction: TranslationDirection,
    original: String,
    preedit: String,
    integrity: Option<IntegrityReport>,
    phase: Phase,
}

impl Active {
    fn is_live(&self) -> bool {
        matches!(
            self.phase,
            Phase::Requested | Phase::Running | Phase::Ready(_)
        )
    }

    fn commit_identity(&self, id: u64) -> CommitIdentity {
        CommitIdentity {
            context: self.context,
            task: self.task,
            commit_id: id,
        }
    }
}

/// 一个控制器拥有唯一的输入引擎。宿主不能绕过翻译事务修改该引擎。
pub struct TranslationBridge {
    engine: InputEngine,
    provider: ProviderIdentity,
    service: TranslationService,
    active: Option<Active>,
    english: String,
    session: u64,
    epoch: u64,
    focused: bool,
    replacing_backend: bool,
}

impl TranslationBridge {
    pub fn new(
        engine: InputEngine,
        session: u64,
        provider: Arc<dyn TranslationProvider>,
        timeout: Duration,
    ) -> Result<Self, BridgeError> {
        if session == 0 || engine.frame().state != InputState::Inactive {
            return Err(BridgeError::InvalidState);
        }
        let (identity, service) = create_service(provider, timeout)?;
        Ok(Self {
            engine,
            provider: identity,
            service,
            active: None,
            english: String::new(),
            session,
            epoch: 1,
            focused: false,
            replacing_backend: false,
        })
    }

    pub fn input(&self) -> &InputEngine {
        &self.engine
    }

    pub fn state(&self) -> TranslationState {
        match self.active.as_ref().map(|a| &a.phase) {
            None => TranslationState::Idle,
            Some(Phase::Requested) => TranslationState::Requested,
            Some(Phase::Running) => TranslationState::Running,
            Some(Phase::Ready(_)) => TranslationState::Ready,
            Some(Phase::Failed(_)) => TranslationState::Failed,
            Some(Phase::Cancelled) => TranslationState::Cancelled,
            Some(Phase::Stale) => TranslationState::Stale,
            Some(Phase::CommitPending { .. }) => TranslationState::CommitPending,
            Some(Phase::RetryReady { .. }) => TranslationState::RetryReady,
            Some(Phase::CommitUnknown { .. }) => TranslationState::CommitUnknown,
            Some(Phase::Completed) => TranslationState::Completed,
        }
    }

    pub fn result(&self) -> Option<&TranslationResult> {
        match &self.active.as_ref()?.phase {
            Phase::Ready(result)
            | Phase::CommitPending { result, .. }
            | Phase::RetryReady { result, .. }
            | Phase::CommitUnknown { result, .. } => Some(result),
            _ => None,
        }
    }

    pub fn integrity_report(&self) -> Option<&IntegrityReport> {
        self.result()?;
        self.active.as_ref()?.integrity.as_ref()
    }

    pub fn source_identity(&self) -> Option<&SourceIdentity> {
        self.active.as_ref().map(|active| &active.source)
    }

    pub fn original(&self) -> Option<&str> {
        self.active.as_ref().map(|a| a.original.as_str())
    }

    pub fn original_preedit(&self) -> Option<&str> {
        self.active.as_ref().map(|a| a.preedit.as_str())
    }

    pub fn failure(&self) -> Option<TranslationError> {
        match self.active.as_ref().map(|a| &a.phase) {
            Some(Phase::Failed(error)) => Some(*error),
            _ => None,
        }
    }

    pub fn english_text(&self) -> &str {
        &self.english
    }

    pub fn apply_input(&mut self, event: InputEvent) -> Result<EventResult, BridgeError> {
        let loses_focus = matches!(event, InputEvent::FocusLost | InputEvent::Deactivate);
        if self.state() == TranslationState::CommitPending && loses_focus {
            self.focused = false;
            self.mark_unknown();
            return Err(BridgeError::OutcomeUnknown);
        }
        let cancels_retry = self.state() == TranslationState::RetryReady
            && (loses_focus || event == InputEvent::Key(Key::Escape));
        if !cancels_retry {
            self.ensure_no_translation_commit()?;
        }
        let before = self.engine.revision();
        if loses_focus {
            self.focused = false;
        } else if event == InputEvent::Activate {
            self.focused = true;
        }
        let update = self.engine.handle(event);
        if cancels_retry {
            self.invalidate(Phase::Cancelled);
        } else if (loses_focus || update.frame.revision != before)
            && self.active.as_ref().is_some_and(Active::is_live)
        {
            self.invalidate(Phase::Stale);
        }
        Ok(update)
    }

    pub fn replace_search_resources(
        &mut self,
        lexicon: Arc<SearchSnapshot>,
        language: Option<Arc<LanguageSnapshot>>,
    ) -> Result<(), BridgeError> {
        self.ensure_no_translation_commit()?;
        self.engine.replace_search_resources(lexicon, language)?;
        self.invalidate_live();
        Ok(())
    }

    pub fn reconfigure_scheme(&mut self, scheme: SchemeConfig) -> Result<(), BridgeError> {
        self.ensure_no_translation_commit()?;
        self.engine.reconfigure_scheme(scheme)?;
        self.invalidate_live();
        Ok(())
    }

    /// 英文由隔离宿主的显式编辑缓冲区提供，系统英文透传语义不变。
    pub fn replace_english(&mut self, text: &str) -> Result<(), BridgeError> {
        self.ensure_no_translation_commit()?;
        if !self.focused || self.engine.frame().mode != InputMode::English {
            return Err(BridgeError::InvalidState);
        }
        if text.len() > TEXT_LIMIT {
            return Err(TranslationError::InputTooLong.into());
        }
        if text.contains('\0') {
            return Err(TranslationError::InvalidInput.into());
        }
        if self.english != text {
            self.invalidate_live();
            self.english = text.to_owned();
        }
        Ok(())
    }

    pub fn request_candidate(
        &mut self,
        revision: u64,
        candidate_id: &str,
    ) -> Result<TranslationTaskId, BridgeError> {
        self.ensure_no_translation_commit()?;
        let frame = self.engine.frame();
        if !self.focused || frame.state != InputState::CandidatesVisible {
            return Err(BridgeError::InvalidState);
        }
        if frame.revision != revision {
            return Err(BridgeError::StaleConfirmation);
        }
        let candidate = frame
            .candidates
            .into_iter()
            .find(|c| c.id == candidate_id)
            .ok_or(BridgeError::StaleConfirmation)?;
        self.request(
            candidate.text,
            frame.preedit,
            Some(candidate.id),
            Language::Chinese,
        )
    }

    pub fn request_english(&mut self) -> Result<TranslationTaskId, BridgeError> {
        self.ensure_no_translation_commit()?;
        let frame = self.engine.frame();
        if !self.focused || frame.state != InputState::Idle || frame.mode != InputMode::English {
            return Err(BridgeError::InvalidState);
        }
        self.request(self.english.clone(), String::new(), None, Language::English)
    }

    fn request(
        &mut self,
        original: String,
        preedit: String,
        candidate_id: Option<String>,
        source: Language,
    ) -> Result<TranslationTaskId, BridgeError> {
        if self.replacing_backend || self.engine.failed_commit().is_some() {
            return Err(BridgeError::InvalidState);
        }
        let direction = TranslationDirection {
            source,
            target: if source == Language::Chinese {
                Language::English
            } else {
                Language::Chinese
            },
        };
        let context = self.current_context();
        let request = TranslationRequest {
            text: original.clone(),
            direction,
            kind: TranslationKind::Sentence,
            context,
            send_authorized_to: None,
            use_cache: false,
        };
        request.validate(TEXT_LIMIT)?;
        self.invalidate_live();
        let task = self.service.submit(&self.provider.id, request)?;
        self.active = Some(Active {
            task,
            context,
            source: self.engine.translation_source_identity(candidate_id),
            direction,
            original,
            preedit,
            integrity: None,
            phase: Phase::Requested,
        });
        Ok(task)
    }

    pub fn poll(&mut self) -> TranslationState {
        let Some(active) = self.active.as_ref() else {
            return TranslationState::Idle;
        };
        if !matches!(active.phase, Phase::Requested | Phase::Running) {
            return self.state();
        }
        if !self.current(active) {
            self.invalidate(Phase::Stale);
            return self.state();
        }
        let task = active.task;
        let phase = match self.service.poll(task) {
            Ok(snapshot) => match snapshot.state {
                TaskState::Queued => Phase::Requested,
                TaskState::Running => Phase::Running,
                TaskState::Cancelled => Phase::Cancelled,
                TaskState::Succeeded => match snapshot.result {
                    Some(result) if self.matches_result(active, &result) => Phase::Ready(result),
                    _ => Phase::Stale,
                },
                TaskState::Failed | TaskState::Expired => {
                    Phase::Failed(snapshot.error.unwrap_or(TranslationError::InternalError))
                }
            },
            Err(error) => Phase::Failed(error),
        };
        if !matches!(phase, Phase::Requested | Phase::Running) {
            let _ = self.service.forget(task);
        }
        let active = self.active.as_mut().expect("当前任务仍由控制器持有");
        if let Phase::Ready(result) = &phase {
            let direction = if result.direction.source == Language::Chinese {
                IntegrityDirection::ChineseToEnglish
            } else {
                IntegrityDirection::EnglishToChinese
            };
            active.integrity = Some(check_translation(
                &active.original,
                &result.output.translated_text,
                direction,
                &Glossary::default(),
            ));
        }
        active.phase = phase;
        self.state()
    }

    pub fn cancel(&mut self) -> Result<(), BridgeError> {
        if self.state() == TranslationState::RetryReady {
            self.engine.handle(InputEvent::Key(Key::Escape));
            self.invalidate(Phase::Cancelled);
            return Ok(());
        }
        self.ensure_no_translation_commit()?;
        if self.active.as_ref().is_some_and(Active::is_live) {
            self.invalidate(Phase::Cancelled);
        }
        Ok(())
    }

    pub fn confirm(
        &mut self,
        context: TranslationContext,
        task: TranslationTaskId,
        result_version: u32,
    ) -> Result<PendingCommit, BridgeError> {
        self.ensure_no_translation_commit()?;
        let active = self.active.as_ref().ok_or(BridgeError::StaleConfirmation)?;
        let Phase::Ready(result) = &active.phase else {
            return Err(BridgeError::StaleConfirmation);
        };
        if context != result.context
            || task != result.task_id
            || result_version != result.result_version
            || !self.current(active)
            || !self.matches_result(active, result)
        {
            return Err(BridgeError::StaleConfirmation);
        }
        if result.output.output_kind != OutputKind::Translation
            || !matches!(
                result.output.quality,
                QualityStatus::Generated | QualityStatus::Approved
            )
        {
            return Err(TranslationError::DataNotApproved.into());
        }
        let update = self
            .engine
            .confirm_translation(context.input_revision, &result.output.translated_text);
        let request = commit_request(update)?;
        let active = self.active.as_mut().expect("确认前已验证当前任务");
        let Phase::Ready(result) = std::mem::replace(&mut active.phase, Phase::Stale) else {
            unreachable!()
        };
        active.phase = Phase::CommitPending {
            id: request.id,
            result,
        };
        Ok(PendingCommit {
            identity: active.commit_identity(request.id),
            request,
        })
    }

    pub fn acknowledge(
        &mut self,
        receipt: CommitIdentity,
        outcome: CommitOutcome,
    ) -> Result<(), BridgeError> {
        let active = self.active.as_ref().ok_or(BridgeError::StaleReceipt)?;
        let Phase::CommitPending { id, .. } = active.phase else {
            return Err(BridgeError::StaleReceipt);
        };
        if active.commit_identity(id) != receipt {
            return Err(BridgeError::StaleReceipt);
        }
        if outcome == CommitOutcome::Unknown {
            self.mark_unknown();
            return Ok(());
        }
        let event = if outcome == CommitOutcome::Applied {
            InputEvent::CommitSucceeded { id }
        } else {
            InputEvent::CommitFailed { id }
        };
        let update = self.engine.handle(event);
        let proven = if outcome == CommitOutcome::Applied {
            update.handled && update.error.is_none()
        } else {
            update
                .error
                .as_ref()
                .is_some_and(|error| error.code == ErrorCode::CommitFailed)
                && self
                    .engine
                    .failed_commit()
                    .is_some_and(|failed| failed.id == id)
        };
        if !proven {
            self.mark_unknown();
            return Err(BridgeError::OutcomeUnknown);
        }
        let active = self.active.as_mut().expect("回执前已验证当前事务");
        if outcome == CommitOutcome::Applied {
            if active.direction.source == Language::English {
                self.english.clear();
            }
            active.original.clear();
            active.preedit.clear();
            active.phase = Phase::Completed;
        } else {
            let Phase::CommitPending { result, .. } =
                std::mem::replace(&mut active.phase, Phase::Stale)
            else {
                unreachable!()
            };
            active.phase = Phase::RetryReady { id, result };
        }
        Ok(())
    }

    pub fn retry(&mut self, previous: CommitIdentity) -> Result<PendingCommit, BridgeError> {
        if !self.focused || self.state() == TranslationState::CommitUnknown {
            return Err(BridgeError::OutcomeUnknown);
        }
        let active = self.active.as_ref().ok_or(BridgeError::StaleReceipt)?;
        let Phase::RetryReady { id, .. } = active.phase else {
            return Err(BridgeError::InvalidState);
        };
        if active.commit_identity(id) != previous
            || self
                .engine
                .failed_commit()
                .is_none_or(|failed| failed.id != id)
        {
            return Err(BridgeError::StaleReceipt);
        }
        let request = commit_request(self.engine.handle(InputEvent::RetryCommit))?;
        let active = self.active.as_mut().expect("重试前已验证当前事务");
        let Phase::RetryReady { result, .. } = std::mem::replace(&mut active.phase, Phase::Stale)
        else {
            unreachable!()
        };
        active.phase = Phase::CommitPending {
            id: request.id,
            result,
        };
        Ok(PendingCommit {
            identity: active.commit_identity(request.id),
            request,
        })
    }

    /// 宿主在核对未知结果并保留需要的恢复材料后，显式换会话。旧事务永远不会重发。
    pub fn reset_after_unknown(
        &mut self,
        receipt: CommitIdentity,
        session: u64,
    ) -> Result<(), BridgeError> {
        let active = self.active.as_ref().ok_or(BridgeError::StaleReceipt)?;
        let Phase::CommitUnknown { id, .. } = active.phase else {
            return Err(BridgeError::InvalidState);
        };
        if active.commit_identity(id) != receipt || session <= self.session {
            return Err(BridgeError::StaleReceipt);
        }
        let epoch = self.epoch.checked_add(1).ok_or(BridgeError::InvalidState)?;
        let result = self.engine.handle(InputEvent::Deactivate);
        if !result.handled || result.error.is_some() {
            return Err(BridgeError::OutcomeUnknown);
        }
        self.session = session;
        self.epoch = epoch;
        self.focused = false;
        self.english.clear();
        self.active = None;
        Ok(())
    }

    pub fn begin_backend_change(&mut self) -> Result<(), BridgeError> {
        self.ensure_no_translation_commit()?;
        self.invalidate_live();
        self.replacing_backend = true;
        self.service.begin_shutdown();
        Ok(())
    }

    pub fn backend_stopped(&self) -> bool {
        self.replacing_backend && self.service.workers_finished()
    }

    /// 仅由允许等待后台退出的宿主调用，常规键入不执行此操作。
    pub fn shutdown(self) -> Result<(), TranslationError> {
        self.service.shutdown()
    }

    pub fn finish_backend_change(
        &mut self,
        provider: Arc<dyn TranslationProvider>,
        timeout: Duration,
    ) -> Result<(), BridgeError> {
        if !self.backend_stopped() {
            return Err(BridgeError::InvalidState);
        }
        self.service.finish_shutdown()?;
        let epoch = self.epoch.checked_add(1).ok_or(BridgeError::InvalidState)?;
        let (identity, service) = create_service(provider, timeout)?;
        self.provider = identity;
        self.service = service;
        self.epoch = epoch;
        self.replacing_backend = false;
        Ok(())
    }

    fn ensure_no_translation_commit(&self) -> Result<(), BridgeError> {
        match self.state() {
            TranslationState::CommitUnknown => Err(BridgeError::OutcomeUnknown),
            TranslationState::CommitPending | TranslationState::RetryReady => {
                Err(BridgeError::InvalidState)
            }
            _ => Ok(()),
        }
    }

    fn current_context(&self) -> TranslationContext {
        TranslationContext {
            session: self.session,
            epoch: self.epoch,
            input_revision: self.engine.revision(),
        }
    }

    fn current(&self, active: &Active) -> bool {
        self.focused
            && active.context == self.current_context()
            && active.source
                == self
                    .engine
                    .translation_source_identity(active.source.candidate_id.clone())
    }

    fn matches_result(&self, active: &Active, result: &TranslationResult) -> bool {
        result.task_id == active.task
            && result.context == active.context
            && result.provider == self.provider
            && result.result_version == RESULT_VERSION
            && result.kind == TranslationKind::Sentence
            && result.direction == active.direction
    }

    fn invalidate_live(&mut self) {
        if self.active.as_ref().is_some_and(Active::is_live) {
            self.invalidate(Phase::Stale);
        }
    }

    fn invalidate(&mut self, phase: Phase) {
        if let Some(active) = &mut self.active {
            if matches!(active.phase, Phase::Requested | Phase::Running) {
                let _ = self.service.forget(active.task);
            }
            active.phase = phase;
        }
    }

    fn mark_unknown(&mut self) {
        let active = self
            .active
            .as_mut()
            .expect("只有已验证的进行中提交会进入未知状态");
        let Phase::CommitPending { id, result } =
            std::mem::replace(&mut active.phase, Phase::Stale)
        else {
            unreachable!()
        };
        active.phase = Phase::CommitUnknown { id, result };
    }
}

fn commit_request(update: EventResult) -> Result<CommitRequest, BridgeError> {
    if let Some(error) = update.error {
        return Err(error.into());
    }
    update.commit.ok_or(BridgeError::InvalidState)
}

fn create_service(
    provider: Arc<dyn TranslationProvider>,
    timeout: Duration,
) -> Result<(ProviderIdentity, TranslationService), BridgeError> {
    if provider.capabilities().network {
        return Err(TranslationError::OnlineNotAuthorized.into());
    }
    let identity = provider.identity();
    let mut registry = ProviderRegistry::new();
    registry.register(
        provider,
        TranslationProviderConfig {
            timeout,
            ..Default::default()
        },
    )?;
    let service = TranslationService::new(
        registry,
        ServiceLimits {
            workers: 1,
            queued: 2,
            retained_tasks: 1,
            max_input_bytes: TEXT_LIMIT,
            max_output_bytes: TEXT_LIMIT,
            cache_entries: 0,
            cache_bytes: 0,
            offline_only: true,
        },
    )?;
    Ok((identity, service))
}
