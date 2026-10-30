use crate::{
    OutputKind, ProviderIdentity, QualityStatus, RESULT_VERSION, TaskSnapshot, TaskState,
    TranslationContext, TranslationDirection, TranslationError, TranslationKind,
    TranslationRequest, TranslationResult, TranslationTaskId,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationPanelState {
    Idle,
    Waiting,
    Ready,
    Failed,
    Cancelled,
    Confirmed,
}

struct Binding {
    task_id: TranslationTaskId,
    provider: ProviderIdentity,
    direction: TranslationDirection,
    kind: TranslationKind,
}

pub struct TranslationPanel {
    context: TranslationContext,
    focused: bool,
    state: TranslationPanelState,
    original: String,
    active: Option<Binding>,
    result: Option<TranslationResult>,
    error: Option<TranslationError>,
}

pub struct TranslationIntent {
    pub task_id: TranslationTaskId,
    pub context: TranslationContext,
    pub text: String,
}

impl fmt::Debug for TranslationIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranslationIntent")
            .field("task_id", &self.task_id)
            .field("context", &self.context)
            .field("text_bytes", &self.text.len())
            .finish_non_exhaustive()
    }
}

impl TranslationPanel {
    pub fn new(context: TranslationContext) -> Self {
        Self {
            context,
            focused: true,
            state: TranslationPanelState::Idle,
            original: String::new(),
            active: None,
            result: None,
            error: None,
        }
    }

    /// 返回旧任务让宿主取消；面板本身不等待后台或调用输入核心。
    pub fn start(
        &mut self,
        task_id: TranslationTaskId,
        request: &TranslationRequest,
        provider: ProviderIdentity,
    ) -> Result<Option<TranslationTaskId>, TranslationError> {
        if !self.focused || request.context != self.context {
            return Err(TranslationError::StaleResult);
        }
        let previous = self.active.take().map(|binding| binding.task_id);
        self.original = request.text.clone();
        self.active = Some(Binding {
            task_id,
            provider,
            direction: request.direction,
            kind: request.kind,
        });
        self.result = None;
        self.error = None;
        self.state = TranslationPanelState::Waiting;
        Ok(previous)
    }

    pub fn update(
        &mut self,
        task_id: TranslationTaskId,
        originating_context: TranslationContext,
        snapshot: TaskSnapshot,
    ) -> Result<(), TranslationError> {
        // 失败回执没有result.context，必须使用请求发起时捕获的上下文。
        if originating_context != self.context {
            return Err(TranslationError::StaleResult);
        }
        let active = self.current(task_id)?;
        if self.state != TranslationPanelState::Waiting {
            return Err(TranslationError::StaleResult);
        }
        if snapshot.state == TaskState::Succeeded {
            let result = snapshot.result.ok_or(TranslationError::InvalidResponse)?;
            if result.task_id != task_id
                || result.context != self.context
                || result.provider != active.provider
                || result.direction != active.direction
                || result.kind != active.kind
                || result.result_version != RESULT_VERSION
            {
                return Err(TranslationError::StaleResult);
            }
            self.result = Some(result);
            self.state = TranslationPanelState::Ready;
        } else if snapshot.state.is_terminal() {
            self.error = snapshot.error;
            self.state = if matches!(snapshot.state, TaskState::Cancelled | TaskState::Expired) {
                TranslationPanelState::Cancelled
            } else {
                TranslationPanelState::Failed
            };
        }
        Ok(())
    }

    pub fn copy(
        &self,
        task_id: TranslationTaskId,
        context: TranslationContext,
        result_version: u32,
    ) -> Result<TranslationIntent, TranslationError> {
        self.current(task_id)?;
        let result = self.ready(context, result_version)?;
        Ok(TranslationIntent {
            task_id,
            context,
            text: result.output.translated_text.clone(),
        })
    }

    pub fn confirm(
        &mut self,
        task_id: TranslationTaskId,
        context: TranslationContext,
        result_version: u32,
    ) -> Result<TranslationIntent, TranslationError> {
        self.current(task_id)?;
        let result = self.ready(context, result_version)?;
        if result.output.output_kind != OutputKind::Translation
            || !matches!(
                result.output.quality,
                QualityStatus::Approved | QualityStatus::Generated
            )
        {
            return Err(TranslationError::DataNotApproved);
        }
        let intent = TranslationIntent {
            task_id,
            context,
            text: result.output.translated_text.clone(),
        };
        // 只消费一次确认；实际上屏仍由平台完成，不能据此重试未知提交。
        self.state = TranslationPanelState::Confirmed;
        self.result = None;
        Ok(intent)
    }

    pub fn cancel(&mut self) -> Option<TranslationTaskId> {
        let old = self.active.take().map(|binding| binding.task_id);
        self.result = None;
        self.error = None;
        self.state = TranslationPanelState::Cancelled;
        old
    }

    pub fn focus_lost(&mut self) -> Option<TranslationTaskId> {
        self.focused = false;
        self.cancel()
    }

    /// 换Service时宿主也必须换session/epoch，TaskId只在单个Service内唯一。
    pub fn focus(&mut self, context: TranslationContext) -> Option<TranslationTaskId> {
        let old = self.cancel();
        self.context = context;
        self.focused = true;
        old
    }

    pub fn state(&self) -> TranslationPanelState {
        self.state
    }
    pub fn original(&self) -> &str {
        &self.original
    }
    pub fn result(&self) -> Option<&TranslationResult> {
        self.result.as_ref()
    }
    pub fn error(&self) -> Option<TranslationError> {
        self.error
    }

    fn current(&self, task_id: TranslationTaskId) -> Result<&Binding, TranslationError> {
        self.active
            .as_ref()
            .filter(|active| self.focused && active.task_id == task_id)
            .ok_or(TranslationError::StaleResult)
    }

    fn ready(
        &self,
        context: TranslationContext,
        version: u32,
    ) -> Result<&TranslationResult, TranslationError> {
        self.result
            .as_ref()
            .filter(|result| {
                self.focused
                    && self.state == TranslationPanelState::Ready
                    && context == self.context
                    && version == result.result_version
            })
            .ok_or(TranslationError::StaleResult)
    }
}

impl fmt::Debug for TranslationPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranslationPanel")
            .field("context", &self.context)
            .field("state", &self.state)
            .field("original_bytes", &self.original.len())
            .finish_non_exhaustive()
    }
}
