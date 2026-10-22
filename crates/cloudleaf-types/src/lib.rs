//! 平台适配器与输入核心共享的事件、候选帧及错误协议。

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidInput,
    InputTooLong,
    InvalidDictionary,
    UnsupportedVersion,
    InvalidConfig,
    CommitPending,
    StaleCommit,
    CommitFailed,
    Io,
}

/// 错误只携带固定说明和行号，不携带原始输入、词条内容或密钥。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreError {
    pub code: ErrorCode,
    pub message: &'static str,
    pub line: Option<usize>,
}

impl CoreError {
    pub const fn new(code: ErrorCode, message: &'static str) -> Self {
        Self {
            code,
            message,
            line: None,
        }
    }

    pub const fn at_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)?;
        if let Some(line) = self.line {
            write!(f, " (line {line})")?;
        }
        Ok(())
    }
}

impl std::error::Error for CoreError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Chinese,
    English,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputState {
    Inactive,
    Idle,
    Composing,
    CandidatesVisible,
    Committing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Backspace,
    Space,
    Enter,
    Escape,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Activate,
    Deactivate,
    FocusLost,
    Key(Key),
    SetMode(InputMode),
    CommitSucceeded { id: u64 },
    CommitFailed { id: u64 },
    RetryCommit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchKind {
    Exact,
    Prefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
    pub text: String,
    pub annotation: Option<String>,
    pub source: String,
    pub frequency: u32,
    pub match_kind: MatchKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRequest {
    pub id: u64,
    pub text: String,
}

/// 候选为有界的完整集合，高亮为全局索引，页码由 page_start/page_size 表示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputFrame {
    pub state: InputState,
    pub mode: InputMode,
    pub revision: u64,
    pub preedit: String,
    pub cursor: usize,
    pub candidates: Vec<Candidate>,
    pub highlighted: Option<usize>,
    pub page_start: usize,
    pub page_size: usize,
}

/// handled=false 时宿主处理原按键；有错误时先处理错误，再决定是否重派按键。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventResult {
    pub handled: bool,
    pub frame: InputFrame,
    pub commit: Option<CommitRequest>,
    pub error: Option<CoreError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_has_stable_code_and_location() {
        let error = CoreError::new(ErrorCode::InvalidDictionary, "invalid field").at_line(7);
        assert_eq!(error.code, ErrorCode::InvalidDictionary);
        assert_eq!(
            error.to_string(),
            "InvalidDictionary: invalid field (line 7)"
        );
        let _: &dyn std::error::Error = &error;
    }
}
