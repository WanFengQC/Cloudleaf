use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

pub const RESULT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Auto,
    Chinese,
    English,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TranslationKind {
    Word,
    Phrase,
    Sentence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TranslationDirection {
    pub source: Language,
    pub target: Language,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TranslationContext {
    pub session: u64,
    pub epoch: u64,
    pub input_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderIdentity {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAuthorization {
    pub provider: ProviderIdentity,
    pub config_version: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct TranslationRequest {
    pub text: String,
    pub direction: TranslationDirection,
    pub kind: TranslationKind,
    pub context: TranslationContext,
    pub send_authorized_to: Option<ProviderAuthorization>,
    pub use_cache: bool,
}

impl TranslationRequest {
    pub fn validate(&self, max_bytes: usize) -> Result<(), TranslationError> {
        if self.text.trim().is_empty() || self.text.contains('\0') {
            return Err(TranslationError::InvalidInput);
        }
        if self.text.len() > max_bytes {
            return Err(TranslationError::InputTooLong);
        }
        if self.direction.target == Language::Auto || self.direction.source == self.direction.target
        {
            return Err(TranslationError::UnsupportedLanguagePair);
        }
        Ok(())
    }
}

impl fmt::Debug for TranslationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranslationRequest")
            .field("text_bytes", &self.text.len())
            .field("direction", &self.direction)
            .field("kind", &self.kind)
            .field("context", &self.context)
            .field("send_authorized", &self.send_authorized_to.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputKind {
    DictionaryGloss,
    ReverseLookup,
    Translation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualityStatus {
    Pending,
    ResearchOnly,
    Approved,
    Generated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadPolicy {
    Research,
    Production,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderOutput {
    pub translated_text: String,
    pub resolved_source: Language,
    pub output_kind: OutputKind,
    pub quality: QualityStatus,
    pub source_ids: Vec<String>,
}

impl fmt::Debug for ProviderOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderOutput")
            .field("text_bytes", &self.translated_text.len())
            .field("resolved_source", &self.resolved_source)
            .field("output_kind", &self.output_kind)
            .field("quality", &self.quality)
            .field("source_count", &self.source_ids.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TranslationTaskId(pub u64);

#[derive(Clone, PartialEq, Eq)]
pub struct TranslationResult {
    pub task_id: TranslationTaskId,
    pub context: TranslationContext,
    pub result_version: u32,
    pub provider: ProviderIdentity,
    pub direction: TranslationDirection,
    pub kind: TranslationKind,
    pub output: ProviderOutput,
    pub from_cache: bool,
}

impl fmt::Debug for TranslationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranslationResult")
            .field("task_id", &self.task_id)
            .field("context", &self.context)
            .field("result_version", &self.result_version)
            .field("output", &self.output)
            .field("from_cache", &self.from_cache)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationError {
    InvalidInput,
    InputTooLong,
    InvalidConfig,
    UnsupportedLanguagePair,
    UnsupportedTaskKind,
    ProviderUnavailable,
    ModelNotFound,
    ModelLoadFailed,
    DataNotApproved,
    NoMatch,
    NotFound,
    Busy,
    OnlineNotAuthorized,
    MissingSecret,
    SecretStoreUnavailable,
    AuthenticationFailed,
    RateLimited { retry_after_seconds: Option<u64> },
    NetworkUnavailable,
    Timeout,
    InvalidResponse,
    ResponseTooLarge,
    Cancelled,
    Expired,
    StaleResult,
    InternalError,
}

impl fmt::Display for TranslationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TranslationError {}

#[derive(Clone)]
pub struct TranslationCancellation(Arc<CancellationState>);

struct CancellationState {
    reason: AtomicU8,
    deadline: Instant,
}

impl TranslationCancellation {
    pub fn new(timeout: Duration) -> Self {
        Self(Arc::new(CancellationState {
            reason: AtomicU8::new(0),
            deadline: Instant::now() + timeout,
        }))
    }

    pub fn cancel(&self) {
        let _ = self
            .0
            .reason
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
    }

    pub fn expire(&self) {
        let _ = self
            .0
            .reason
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.reason.load(Ordering::Acquire) != 0
    }

    pub fn remaining(&self) -> Duration {
        self.0.deadline.saturating_duration_since(Instant::now())
    }

    pub fn check(&self) -> Result<(), TranslationError> {
        match self.0.reason.load(Ordering::Acquire) {
            1 => Err(TranslationError::Cancelled),
            2 => Err(TranslationError::Expired),
            _ if self.remaining().is_zero() => Err(TranslationError::Timeout),
            _ => Ok(()),
        }
    }
}

impl fmt::Debug for TranslationCancellation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranslationCancellation")
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Expired,
}

impl TaskState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Expired
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    pub state: TaskState,
    pub result: Option<TranslationResult>,
    pub error: Option<TranslationError>,
}
