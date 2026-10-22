//! 平台无关输入状态机；宿主负责执行提交请求并同步确认结果。

use cloudleaf_dictionary::Dictionary;
use cloudleaf_types::{
    Candidate, CommitRequest, CoreError, ErrorCode, EventResult, InputEvent, InputFrame, InputMode,
    InputState, Key, KeyDisposition,
};

/// 首期每页最多显示五个候选，数字快捷键按当前页编号。
pub const DEFAULT_PAGE_SIZE: usize = 5;
/// 单次生成的候选总数上限。
pub const MAX_CANDIDATES: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputConfig {
    pub page_size: usize,
    pub max_candidates: usize,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            page_size: DEFAULT_PAGE_SIZE,
            max_candidates: MAX_CANDIDATES,
        }
    }
}

/// 拥有当前组合上下文的同步输入引擎。
pub struct InputEngine {
    config: InputConfig,
    dictionary: Dictionary,
    state: InputState,
    mode: InputMode,
    revision: u64,
    preedit: String,
    cursor: usize,
    candidates: Vec<Candidate>,
    highlighted: Option<usize>,
    page_start: usize,
    next_commit_id: u64,
    pending_commit: Option<CommitRequest>,
    failed_commit: Option<CommitRequest>,
}

impl InputEngine {
    /// 新引擎处于未激活状态，使用中文模式；词典由宿主在按键路径外加载。
    pub fn new(dictionary: Dictionary) -> Self {
        Self::with_config(dictionary, InputConfig::default()).expect("默认输入配置必须有效")
    }

    pub fn with_config(dictionary: Dictionary, config: InputConfig) -> Result<Self, CoreError> {
        if !(1..=9).contains(&config.page_size)
            || config.max_candidates == 0
            || config.max_candidates > cloudleaf_candidate::MAX_CANDIDATES
        {
            return Err(CoreError::new(
                ErrorCode::InvalidConfig,
                "候选页大小或候选预算无效",
            ));
        }
        Ok(Self {
            config,
            dictionary,
            state: InputState::Inactive,
            mode: InputMode::Chinese,
            revision: 0,
            preedit: String::new(),
            cursor: 0,
            candidates: Vec::new(),
            highlighted: None,
            page_start: 0,
            next_commit_id: 1,
            pending_commit: None,
            failed_commit: None,
        })
    }

    /// 返回独立的显示快照；候选索引指向完整候选数组。
    pub fn frame(&self) -> InputFrame {
        InputFrame {
            state: self.state,
            mode: self.mode,
            revision: self.revision,
            preedit: self.preedit.clone(),
            cursor: self.cursor,
            candidates: self.candidates.clone(),
            highlighted: self.highlighted,
            page_start: self.page_start,
            page_size: self.config.page_size,
        }
    }

    /// 最近一次失败请求供宿主提示或显式恢复；不得自动重发。
    ///
    /// 失败后普通按键暂停处理，直到宿主明确重试或取消，记录不会写入日志。
    pub fn failed_commit(&self) -> Option<&CommitRequest> {
        self.failed_commit.as_ref()
    }

    /// 预判不执行handle、不解析词典、不生成快照；系统修饰键由平台先行过滤。
    pub fn key_disposition(&self, key: Key) -> KeyDisposition {
        use KeyDisposition::{Blocked, Handle, PassThrough};
        if self.revision == u64::MAX {
            return Blocked(ErrorCode::InvalidInput);
        }
        if self.state == InputState::Inactive {
            return PassThrough;
        }
        if key == Key::Escape && (self.pending_commit.is_some() || self.failed_commit.is_some()) {
            return Handle;
        }
        if self.pending_commit.is_some() {
            return Blocked(ErrorCode::CommitPending);
        }
        if self.failed_commit.is_some() {
            return Blocked(ErrorCode::CommitFailed);
        }
        if self.mode == InputMode::English {
            return PassThrough;
        }
        let composing = !self.preedit.is_empty();
        let handle_composition = if composing { Handle } else { PassThrough };
        let disposition = match key {
            Key::Char(' ' | '\n') | Key::Space | Key::Enter => handle_composition,
            Key::Char(character)
                if character.is_ascii_alphabetic() || (character == '\'' && composing) =>
            {
                if self.preedit.len() >= cloudleaf_pinyin::MAX_INPUT_LEN {
                    Blocked(ErrorCode::InputTooLong)
                } else {
                    Handle
                }
            }
            Key::Char(character) if character.is_control() => Blocked(ErrorCode::InvalidInput),
            Key::Char(character) if chinese_punctuation(character).is_some() => Handle,
            Key::Char(_) | Key::Backspace | Key::Escape | Key::Left | Key::Right => {
                handle_composition
            }
            Key::Up | Key::Down | Key::PageUp | Key::PageDown => {
                if self.candidates.is_empty() {
                    PassThrough
                } else {
                    Handle
                }
            }
        };
        // 提交序号耗尽时，仅阻止会创建提交的键；光标编辑和取消仍可执行。
        let creates_commit = match key {
            Key::Char(' ' | '\n') | Key::Space | Key::Enter => composing,
            Key::Char(character)
                if character.is_ascii_alphabetic() || (character == '\'' && composing) =>
            {
                false
            }
            Key::Char(character) if character.is_control() => false,
            Key::Char(character) => composing || chinese_punctuation(character).is_some(),
            _ => false,
        };
        if disposition == Handle && creates_commit && self.next_commit_id == u64::MAX {
            Blocked(ErrorCode::InvalidInput)
        } else {
            disposition
        }
    }

    /// 鼠标/无障碍选择必须绑定当前帧，不允许平台提交显示缓存中的旧文本。
    pub fn select_candidate(&mut self, revision: u64, id: &str) -> EventResult {
        let error = if self.pending_commit.is_some() {
            Some(ErrorCode::CommitPending)
        } else if self.failed_commit.is_some() {
            Some(ErrorCode::CommitFailed)
        } else if self.revision == u64::MAX || self.state == InputState::Inactive {
            Some(ErrorCode::InvalidInput)
        } else if revision != self.revision {
            Some(ErrorCode::StaleCandidate)
        } else {
            None
        };
        if let Some(code) = error {
            return self.result(
                false,
                None,
                Some(CoreError::new(code, "候选选择不属于当前可提交帧")),
            );
        }
        let Some(candidate) = self.candidates.iter().find(|candidate| candidate.id == id) else {
            return self.result(
                false,
                None,
                Some(CoreError::new(
                    ErrorCode::UnknownCandidate,
                    "候选标识不存在",
                )),
            );
        };
        self.begin_commit(candidate.text.clone())
    }

    /// 处理一个事件，不执行文件读写、网络访问或系统文本提交。
    ///
    /// `commit` 只在首次请求时返回。宿主应先执行该请求并确认成功或失败，
    /// 再分派下一按键；等待期间的普通按键返回 `CommitPending`，供宿主保留并重派。
    pub fn handle(&mut self, event: InputEvent) -> EventResult {
        // 耗尽后停止改变上下文，不能饱和后继续复用同一修订号。
        if self.revision == u64::MAX {
            return self.result(
                false,
                None,
                Some(CoreError::new(
                    ErrorCode::InvalidInput,
                    "输入修订号已耗尽，需要新建输入上下文",
                )),
            );
        }
        match event {
            InputEvent::Activate => {
                if self.state == InputState::Inactive {
                    self.clear_context(InputState::Idle);
                }
                self.result(true, None, None)
            }
            InputEvent::Deactivate => {
                self.clear_context(InputState::Inactive);
                self.result(true, None, None)
            }
            InputEvent::FocusLost => {
                self.cancel();
                self.result(true, None, None)
            }
            InputEvent::SetMode(mode) => {
                if self.mode != mode {
                    self.cancel();
                    self.mode = mode;
                }
                self.result(true, None, None)
            }
            InputEvent::CommitSucceeded { id } => self.acknowledge_commit(id, true),
            InputEvent::CommitFailed { id } => self.acknowledge_commit(id, false),
            InputEvent::RetryCommit => self.retry_commit(),
            InputEvent::Key(key) => self.handle_key(key),
        }
    }

    fn handle_key(&mut self, key: Key) -> EventResult {
        if self.state == InputState::Inactive {
            return self.result(false, None, None);
        }

        // 取消始终使提交标识失效，迟到的确认不能影响下一轮输入。
        if key == Key::Escape && (self.pending_commit.is_some() || self.failed_commit.is_some()) {
            self.cancel();
            return self.result(true, None, None);
        }
        if self.pending_commit.is_some() {
            return self.result(
                false,
                None,
                Some(CoreError::new(
                    ErrorCode::CommitPending,
                    "宿主必须先确认当前提交，再分派下一按键",
                )),
            );
        }
        if self.failed_commit.is_some() {
            return self.result(
                false,
                None,
                Some(CoreError::new(
                    ErrorCode::CommitFailed,
                    "提交失败，需要显式重试或取消后再分派按键",
                )),
            );
        }
        if self.mode == InputMode::English {
            return self.result(false, None, None);
        }

        match key {
            Key::Char(' ') | Key::Space | Key::Char('\n') | Key::Enter => {
                if self.preedit.is_empty() {
                    self.result(false, None, None)
                } else {
                    self.begin_commit(self.confirmed_text())
                }
            }
            Key::Char(character) => self.handle_character(character),
            Key::Backspace => {
                if self.preedit.is_empty() {
                    return self.result(false, None, None);
                }
                if self.cursor > 0 {
                    self.failed_commit = None;
                    self.cursor -= 1;
                    self.preedit.remove(self.cursor);
                    self.refresh();
                }
                self.result(true, None, None)
            }
            Key::Escape => {
                if self.preedit.is_empty() {
                    self.result(false, None, None)
                } else {
                    self.cancel();
                    self.result(true, None, None)
                }
            }
            Key::Left | Key::Right => {
                if self.preedit.is_empty() {
                    return self.result(false, None, None);
                }
                let previous = self.cursor;
                self.cursor = if key == Key::Left {
                    self.cursor.saturating_sub(1)
                } else {
                    (self.cursor + 1).min(self.preedit.len())
                };
                if previous != self.cursor {
                    self.advance_revision();
                }
                self.result(true, None, None)
            }
            Key::Up | Key::Down | Key::PageUp | Key::PageDown => self.navigate(key),
        }
    }

    fn handle_character(&mut self, character: char) -> EventResult {
        if character.is_ascii_alphabetic() || (character == '\'' && !self.preedit.is_empty()) {
            if self.preedit.len() >= cloudleaf_pinyin::MAX_INPUT_LEN {
                return self.result(
                    false,
                    None,
                    Some(CoreError::new(
                        ErrorCode::InputTooLong,
                        "组合输入已达到长度上限",
                    )),
                );
            }
            self.failed_commit = None;
            self.preedit.insert(self.cursor, character);
            self.cursor += 1;
            self.refresh();
            return self.result(true, None, None);
        }

        if let Some(digit) = character.to_digit(10)
            && character.is_ascii_digit()
            && (1..=9).contains(&digit)
            && !self.candidates.is_empty()
        {
            let index = self.page_start + digit as usize - 1;
            let page_end = (self.page_start + self.config.page_size).min(self.candidates.len());
            if index < page_end {
                return self.begin_commit(self.candidates[index].text.clone());
            }
        }

        if character.is_control() {
            return self.result(
                false,
                None,
                Some(CoreError::new(
                    ErrorCode::InvalidInput,
                    "控制字符应由宿主转换为明确按键事件",
                )),
            );
        }

        if let Some(punctuation) = chinese_punctuation(character) {
            let mut text = self.confirmed_text();
            text.push(punctuation);
            return self.begin_commit(text);
        }
        if self.preedit.is_empty() {
            self.result(false, None, None)
        } else {
            // 非快捷数字和未知字符保留原始拼音，不能静默吞掉字符。
            let mut text = self.preedit.clone();
            text.push(character);
            self.begin_commit(text)
        }
    }

    fn navigate(&mut self, key: Key) -> EventResult {
        let Some(current) = self.highlighted else {
            return self.result(false, None, None);
        };
        let last = self.candidates.len() - 1;
        let next = match key {
            Key::Up => current.saturating_sub(1),
            Key::Down => (current + 1).min(last),
            Key::PageUp => self.page_start.saturating_sub(self.config.page_size),
            Key::PageDown => (self.page_start + self.config.page_size)
                .min(last / self.config.page_size * self.config.page_size),
            _ => current,
        };
        if next != current {
            self.highlighted = Some(next);
            self.page_start = next / self.config.page_size * self.config.page_size;
            self.advance_revision();
        }
        self.result(true, None, None)
    }

    fn confirmed_text(&self) -> String {
        self.highlighted
            .and_then(|index| self.candidates.get(index))
            .map_or_else(|| self.preedit.clone(), |candidate| candidate.text.clone())
    }

    fn begin_commit(&mut self, text: String) -> EventResult {
        let Some(next_id) = self.next_commit_id.checked_add(1) else {
            return self.result(
                false,
                None,
                Some(CoreError::new(
                    ErrorCode::InvalidInput,
                    "提交序号已耗尽，需要新建输入上下文",
                )),
            );
        };
        let request = CommitRequest {
            id: self.next_commit_id,
            text,
        };
        self.next_commit_id = next_id;
        self.failed_commit = None;
        self.pending_commit = Some(request.clone());
        self.state = InputState::Committing;
        self.advance_revision();
        self.result(true, Some(request), None)
    }

    fn acknowledge_commit(&mut self, id: u64, success: bool) -> EventResult {
        if self.pending_commit.as_ref().map(|request| request.id) != Some(id) {
            return self.result(
                false,
                None,
                Some(CoreError::new(
                    ErrorCode::StaleCommit,
                    "提交确认不属于当前输入上下文",
                )),
            );
        }
        if success {
            self.clear_context(InputState::Idle);
            self.result(true, None, None)
        } else {
            self.failed_commit = self.pending_commit.take();
            self.state = self.composition_state();
            self.advance_revision();
            self.result(
                true,
                None,
                Some(CoreError::new(
                    ErrorCode::CommitFailed,
                    "目标应用未确认提交，已保留组合输入且不会自动重发",
                )),
            )
        }
    }

    fn retry_commit(&mut self) -> EventResult {
        if self.pending_commit.is_some() {
            return self.result(
                false,
                None,
                Some(CoreError::new(
                    ErrorCode::CommitPending,
                    "当前提交仍在等待宿主确认",
                )),
            );
        }
        if let Some(failed) = self.failed_commit.as_ref() {
            self.begin_commit(failed.text.clone())
        } else {
            self.result(
                false,
                None,
                Some(CoreError::new(
                    ErrorCode::CommitFailed,
                    "当前上下文没有可重试的失败提交",
                )),
            )
        }
    }

    fn refresh(&mut self) {
        self.candidates = cloudleaf_pinyin::parse(&self.preedit).map_or_else(
            |_| Vec::new(),
            |parsed| {
                cloudleaf_candidate::generate(&parsed, &self.dictionary, self.config.max_candidates)
            },
        );
        self.highlighted = (!self.candidates.is_empty()).then_some(0);
        self.page_start = 0;
        self.state = self.composition_state();
        self.advance_revision();
    }

    fn composition_state(&self) -> InputState {
        if self.preedit.is_empty() {
            InputState::Idle
        } else if self.candidates.is_empty() {
            InputState::Composing
        } else {
            InputState::CandidatesVisible
        }
    }

    fn cancel(&mut self) {
        let next = if self.state == InputState::Inactive {
            InputState::Inactive
        } else {
            InputState::Idle
        };
        self.clear_context(next);
    }

    fn clear_context(&mut self, state: InputState) {
        self.preedit.clear();
        self.cursor = 0;
        self.candidates.clear();
        self.highlighted = None;
        self.page_start = 0;
        self.pending_commit = None;
        self.failed_commit = None;
        self.state = state;
        self.advance_revision();
    }

    fn advance_revision(&mut self) {
        // handle 在进入前检查上限，每个事件至多增加一次修订号。
        self.revision += 1;
    }

    fn result(
        &self,
        handled: bool,
        commit: Option<CommitRequest>,
        error: Option<CoreError>,
    ) -> EventResult {
        EventResult {
            handled,
            frame: self.frame(),
            commit,
            error,
        }
    }
}

fn chinese_punctuation(character: char) -> Option<char> {
    match character {
        ',' => Some('，'),
        '.' => Some('。'),
        '?' => Some('？'),
        '!' => Some('！'),
        ':' => Some('：'),
        ';' => Some('；'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_engine() -> InputEngine {
        let mut tsv = String::from(
            "# cloudleaf-dictionary-v1\nid\tpinyin\ttext\tfrequency\tannotation\tsource\n",
        );
        for (index, text) in [
            "你", "尼", "泥", "拟", "逆", "倪", "妮", "腻", "匿", "霓", "溺",
        ]
        .iter()
        .enumerate()
        {
            tsv.push_str(&format!(
                "ni-{index:02}\tni\t{text}\t{}\tyou\tproject-test\n",
                100 - index,
            ));
        }
        tsv.push_str("hello\tni hao\t你好\t200\thello\tproject-test\n");
        let mut engine = InputEngine::new(Dictionary::from_tsv(&tsv).unwrap());
        engine.handle(InputEvent::Activate);
        engine
    }

    fn type_text(engine: &mut InputEngine, text: &str) {
        for character in text.chars() {
            let result = engine.handle(InputEvent::Key(Key::Char(character)));
            assert!(result.handled);
            assert!(result.error.is_none());
        }
    }

    fn key(engine: &mut InputEngine, key: Key) -> EventResult {
        engine.handle(InputEvent::Key(key))
    }

    #[test]
    fn activation_is_required_and_idempotent() {
        let mut engine = InputEngine::new(Dictionary::empty());
        assert!(!key(&mut engine, Key::Char('n')).handled);
        assert_eq!(engine.frame().state, InputState::Inactive);
        engine.handle(InputEvent::Activate);
        type_text(&mut engine, "ni");
        engine.handle(InputEvent::Activate);
        assert_eq!(engine.frame().preedit, "ni");
        engine.handle(InputEvent::Deactivate);
        assert_eq!(engine.frame().state, InputState::Inactive);
        assert!(engine.frame().preedit.is_empty());
    }

    #[test]
    fn space_and_enter_commit_once_without_extra_text_or_annotation() {
        for confirm_key in [Key::Space, Key::Enter] {
            let mut engine = new_engine();
            type_text(&mut engine, "nihao");
            let result = key(&mut engine, confirm_key);
            assert!(result.handled);
            assert_eq!(result.frame.state, InputState::Committing);
            assert_eq!(result.frame.preedit, "nihao");
            let request = result.commit.unwrap();
            assert_eq!(request.text, "你好");
            let repeated = key(&mut engine, confirm_key);
            assert!(!repeated.handled);
            assert!(repeated.commit.is_none());
            assert_eq!(repeated.error.unwrap().code, ErrorCode::CommitPending);
            let accepted = engine.handle(InputEvent::CommitSucceeded { id: request.id });
            assert_eq!(accepted.frame.state, InputState::Idle);
            assert!(accepted.frame.preedit.is_empty());
            assert!(accepted.frame.candidates.is_empty());
            assert!(!key(&mut engine, confirm_key).handled);
        }
    }

    #[test]
    fn no_dictionary_or_no_match_falls_back_to_exact_raw_input() {
        for dictionary in [
            Dictionary::empty(),
            Dictionary::from_tsv(cloudleaf_dictionary::PROTOTYPE_TSV).unwrap(),
        ] {
            let mut engine = InputEngine::new(dictionary);
            engine.handle(InputEvent::Activate);
            type_text(&mut engine, "Zzzq");
            let result = key(&mut engine, Key::Enter);
            assert_eq!(result.commit.unwrap().text, "Zzzq");
            assert!(result.handled);
        }
    }

    #[test]
    fn cursor_edits_and_backspace_operate_before_the_cursor() {
        let mut engine = new_engine();
        type_text(&mut engine, "nhao");
        for _ in 0..3 {
            key(&mut engine, Key::Left);
        }
        type_text(&mut engine, "i");
        assert_eq!(engine.frame().preedit, "nihao");
        assert_eq!(engine.frame().cursor, 2);
        assert_eq!(engine.frame().candidates[0].text, "你好");
        key(&mut engine, Key::Backspace);
        assert_eq!(engine.frame().preedit, "nhao");
        assert_eq!(engine.frame().cursor, 1);
        key(&mut engine, Key::Left);
        assert!(key(&mut engine, Key::Backspace).handled);
        assert_eq!(engine.frame().preedit, "nhao");
        for _ in 0..8 {
            key(&mut engine, Key::Right);
        }
        for _ in 0..4 {
            key(&mut engine, Key::Backspace);
        }
        assert_eq!(engine.frame().state, InputState::Idle);
        assert!(!key(&mut engine, Key::Backspace).handled);
    }

    #[test]
    fn selection_and_page_numbering_are_bounded_to_current_page() {
        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        assert_eq!(engine.frame().highlighted, Some(0));
        key(&mut engine, Key::PageDown);
        assert_eq!(engine.frame().page_start, 5);
        assert_eq!(engine.frame().highlighted, Some(5));
        let result = key(&mut engine, Key::Char('2'));
        assert_eq!(result.commit.unwrap().text, "妮");

        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        key(&mut engine, Key::PageDown);
        key(&mut engine, Key::PageDown);
        assert_eq!(engine.frame().page_start, 10);
        assert_eq!(engine.frame().highlighted, Some(10));
        key(&mut engine, Key::Down);
        assert_eq!(engine.frame().highlighted, Some(10));
        let result = key(&mut engine, Key::Char('2'));
        assert_eq!(result.commit.unwrap().text, "ni2");
    }

    #[test]
    fn unavailable_digit_and_zero_preserve_raw_text_and_character() {
        for digit in ['0', '6', '9'] {
            let mut engine = new_engine();
            type_text(&mut engine, "ni");
            let result = key(&mut engine, Key::Char(digit));
            assert_eq!(result.commit.unwrap().text, format!("ni{digit}"));
        }
        let mut engine = new_engine();
        assert!(!key(&mut engine, Key::Char('1')).handled);
        type_text(&mut engine, "zz");
        assert_eq!(key(&mut engine, Key::Char('1')).commit.unwrap().text, "zz1");
    }

    #[test]
    fn navigation_crosses_pages_and_clamps_at_both_ends() {
        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        key(&mut engine, Key::PageDown);
        key(&mut engine, Key::PageDown);
        key(&mut engine, Key::PageUp);
        assert_eq!(engine.frame().highlighted, Some(5));
        assert_eq!(engine.frame().page_start, 5);
        key(&mut engine, Key::Up);
        assert_eq!(engine.frame().highlighted, Some(4));
        assert_eq!(engine.frame().page_start, 0);
        for _ in 0..20 {
            key(&mut engine, Key::Up);
        }
        key(&mut engine, Key::PageUp);
        assert_eq!(engine.frame().highlighted, Some(0));
        assert_eq!(engine.frame().page_start, 0);
        for _ in 0..20 {
            key(&mut engine, Key::Down);
        }
        key(&mut engine, Key::PageDown);
        assert_eq!(engine.frame().highlighted, Some(10));
        assert_eq!(engine.frame().page_start, 10);
    }

    #[test]
    fn punctuation_uses_selected_candidate_and_unknown_unicode_keeps_raw() {
        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        key(&mut engine, Key::Down);
        assert_eq!(
            key(&mut engine, Key::Char(',')).commit.unwrap().text,
            "尼，"
        );
        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        assert_eq!(
            key(&mut engine, Key::Char('🙂')).commit.unwrap().text,
            "ni🙂"
        );
        let mut engine = new_engine();
        assert_eq!(key(&mut engine, Key::Char('.')).commit.unwrap().text, "。");
    }

    #[test]
    fn cancel_focus_loss_deactivation_and_mode_switch_clear_old_context() {
        for event in [
            InputEvent::Key(Key::Escape),
            InputEvent::FocusLost,
            InputEvent::Deactivate,
            InputEvent::SetMode(InputMode::English),
        ] {
            let mut engine = new_engine();
            type_text(&mut engine, "ni");
            let pending = key(&mut engine, Key::Space).commit.unwrap();
            let result = engine.handle(event);
            assert!(result.frame.preedit.is_empty());
            assert!(result.frame.candidates.is_empty());
            assert!(result.commit.is_none());
            let stale = engine.handle(InputEvent::CommitSucceeded { id: pending.id });
            assert_eq!(stale.error.unwrap().code, ErrorCode::StaleCommit);
            assert!(stale.frame.preedit.is_empty());
        }
    }

    #[test]
    fn stale_confirmation_cannot_clear_new_composition_or_new_request() {
        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        let first = key(&mut engine, Key::Space).commit.unwrap();
        key(&mut engine, Key::Escape);
        type_text(&mut engine, "nihao");
        let before = engine.frame();
        let stale = engine.handle(InputEvent::CommitFailed { id: first.id });
        assert_eq!(stale.error.unwrap().code, ErrorCode::StaleCommit);
        assert_eq!(stale.frame.preedit, before.preedit);
        assert_eq!(stale.frame.revision, before.revision);
        let second = key(&mut engine, Key::Enter).commit.unwrap();
        assert_ne!(first.id, second.id);
        let stale = engine.handle(InputEvent::CommitSucceeded { id: first.id });
        assert_eq!(stale.frame.state, InputState::Committing);
        engine.handle(InputEvent::CommitSucceeded { id: second.id });
        assert_eq!(engine.frame().state, InputState::Idle);
    }

    #[test]
    fn failed_commit_preserves_selection_and_complete_failed_request() {
        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        key(&mut engine, Key::Down);
        let request = key(&mut engine, Key::Char('!')).commit.unwrap();
        let result = engine.handle(InputEvent::CommitFailed { id: request.id });
        assert_eq!(result.error.unwrap().code, ErrorCode::CommitFailed);
        assert!(result.commit.is_none());
        assert_eq!(result.frame.preedit, "ni");
        assert_eq!(result.frame.highlighted, Some(1));
        assert_eq!(result.frame.state, InputState::CandidatesVisible);
        assert_eq!(engine.failed_commit().unwrap().text, "尼！");
        let rejected = key(&mut engine, Key::Char('h'));
        assert!(!rejected.handled);
        assert_eq!(rejected.error.unwrap().code, ErrorCode::CommitFailed);
        assert_eq!(rejected.frame.preedit, "ni");
        assert_eq!(engine.failed_commit().unwrap().text, "尼！");
        let enter = key(&mut engine, Key::Enter);
        assert!(!enter.handled);
        assert!(enter.commit.is_none());
        let retry = engine.handle(InputEvent::RetryCommit).commit.unwrap();
        assert_eq!(retry.text, "尼！");
        assert_ne!(retry.id, request.id);
        assert!(engine.failed_commit().is_none());
        let stale = engine.handle(InputEvent::CommitSucceeded { id: request.id });
        assert_eq!(stale.error.unwrap().code, ErrorCode::StaleCommit);
        assert_eq!(stale.frame.state, InputState::Committing);
        engine.handle(InputEvent::CommitSucceeded { id: retry.id });
        assert_eq!(engine.frame().state, InputState::Idle);
    }

    #[test]
    fn punctuation_only_failure_remains_recoverable_until_explicit_cancel() {
        let mut engine = new_engine();
        let first = key(&mut engine, Key::Char('.')).commit.unwrap();
        engine.handle(InputEvent::CommitFailed { id: first.id });
        assert_eq!(engine.failed_commit().unwrap().text, "。");
        assert!(!key(&mut engine, Key::Char('n')).handled);
        assert!(key(&mut engine, Key::Escape).handled);
        assert!(engine.failed_commit().is_none());
        type_text(&mut engine, "ni");
        assert_eq!(engine.frame().candidates[0].text, "你");
        let retry = engine.handle(InputEvent::RetryCommit);
        assert!(!retry.handled);
        assert!(retry.commit.is_none());
        assert_eq!(retry.error.unwrap().code, ErrorCode::CommitFailed);
        assert_eq!(retry.frame.preedit, "ni");
    }

    #[test]
    fn english_mode_and_idle_system_keys_are_passed_through() {
        let mut engine = new_engine();
        for pass_key in [
            Key::Enter,
            Key::Space,
            Key::Backspace,
            Key::Escape,
            Key::Left,
            Key::Up,
        ] {
            assert!(!key(&mut engine, pass_key).handled);
        }
        type_text(&mut engine, "ni");
        engine.handle(InputEvent::SetMode(InputMode::English));
        for pass_key in [Key::Char('A'), Key::Char(','), Key::Space, Key::Enter] {
            let result = key(&mut engine, pass_key);
            assert!(!result.handled);
            assert!(result.commit.is_none());
            assert!(result.frame.preedit.is_empty());
        }
        engine.handle(InputEvent::SetMode(InputMode::Chinese));
        type_text(&mut engine, "ni");
        assert_eq!(engine.frame().candidates[0].text, "你");
    }

    #[test]
    fn overlong_input_and_control_character_do_not_mutate_composition() {
        let mut engine = InputEngine::new(Dictionary::empty());
        engine.handle(InputEvent::Activate);
        type_text(&mut engine, &"q".repeat(cloudleaf_pinyin::MAX_INPUT_LEN));
        let previous = engine.frame();
        let rejected = key(&mut engine, Key::Char('a'));
        assert!(!rejected.handled);
        assert_eq!(rejected.error.unwrap().code, ErrorCode::InputTooLong);
        assert_eq!(rejected.frame.preedit, previous.preedit);
        assert_eq!(rejected.frame.revision, previous.revision);
        let control = key(&mut engine, Key::Char('\0'));
        assert_eq!(control.error.unwrap().code, ErrorCode::InvalidInput);
        assert_eq!(control.frame.preedit, previous.preedit);
    }

    #[test]
    fn text_change_resets_highlight_and_page_to_current_candidates() {
        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        key(&mut engine, Key::PageDown);
        key(&mut engine, Key::PageDown);
        let revision = engine.frame().revision;
        type_text(&mut engine, "hao");
        let frame = engine.frame();
        assert!(frame.revision > revision);
        assert_eq!(frame.page_start, 0);
        assert_eq!(frame.highlighted, Some(0));
        assert_eq!(frame.candidates.len(), 1);
        assert_eq!(frame.candidates[0].text, "你好");
    }

    #[test]
    fn deterministic_mixed_events_preserve_state_invariants() {
        let mut engine = new_engine();
        let mut seed = 0x1234_5678_u64;
        let mut pending_id = None;
        for _ in 0..2_000 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let event = match seed % 21 {
                0 => InputEvent::FocusLost,
                1 => InputEvent::Deactivate,
                2 => InputEvent::Activate,
                3 => InputEvent::SetMode(InputMode::English),
                4 => InputEvent::SetMode(InputMode::Chinese),
                5 => InputEvent::Key(Key::Escape),
                6 => InputEvent::Key(Key::Backspace),
                7 => InputEvent::Key(Key::Left),
                8 => InputEvent::Key(Key::Right),
                9 => InputEvent::Key(Key::Down),
                10 => InputEvent::Key(Key::PageDown),
                11 => InputEvent::Key(Key::Enter),
                12 => InputEvent::Key(Key::Char('2')),
                13 => InputEvent::Key(Key::Char(',')),
                14 => InputEvent::CommitSucceeded {
                    id: pending_id.unwrap_or(0),
                },
                15 => InputEvent::CommitFailed {
                    id: pending_id.unwrap_or(0),
                },
                16 => InputEvent::RetryCommit,
                _ => InputEvent::Key(Key::Char(
                    ['n', 'i', 'h', 'a', 'o'][(seed as usize >> 8) % 5],
                )),
            };
            let result = engine.handle(event);
            if let Some(request) = result.commit {
                pending_id = Some(request.id);
            }
            let frame = result.frame;
            assert!(frame.preedit.is_ascii());
            assert!(frame.preedit.len() <= cloudleaf_pinyin::MAX_INPUT_LEN);
            assert!(frame.cursor <= frame.preedit.len());
            assert!(frame.candidates.len() <= MAX_CANDIDATES);
            assert!(
                frame
                    .highlighted
                    .is_none_or(|index| index < frame.candidates.len())
            );
            if let Some(index) = frame.highlighted {
                assert!((frame.page_start..frame.page_start + frame.page_size).contains(&index));
            }
            if matches!(frame.state, InputState::Inactive | InputState::Idle) {
                assert!(frame.preedit.is_empty());
                assert!(frame.candidates.is_empty());
                assert!(frame.highlighted.is_none());
            }
            if frame.state == InputState::CandidatesVisible {
                assert!(!frame.preedit.is_empty());
                assert!(!frame.candidates.is_empty());
            }
        }
    }

    #[test]
    fn exhausted_revision_or_commit_ids_are_not_reused() {
        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        engine.revision = u64::MAX - 1;
        let final_revision = key(&mut engine, Key::Left);
        assert_eq!(final_revision.frame.revision, u64::MAX);
        let before = engine.frame();
        let exhausted = key(&mut engine, Key::Right);
        assert!(!exhausted.handled);
        assert!(exhausted.commit.is_none());
        assert_eq!(exhausted.error.unwrap().code, ErrorCode::InvalidInput);
        assert_eq!(exhausted.frame, before);

        let mut engine = new_engine();
        type_text(&mut engine, "ni");
        engine.next_commit_id = u64::MAX - 1;
        let final_commit = key(&mut engine, Key::Enter).commit.unwrap();
        assert_eq!(final_commit.id, u64::MAX - 1);
        engine.handle(InputEvent::CommitSucceeded {
            id: final_commit.id,
        });
        type_text(&mut engine, "ni");
        let before = engine.frame();
        assert_eq!(
            engine.key_disposition(Key::Enter),
            KeyDisposition::Blocked(ErrorCode::InvalidInput)
        );
        assert_eq!(engine.key_disposition(Key::Left), KeyDisposition::Handle);
        assert_eq!(
            engine.key_disposition(Key::Char('a')),
            KeyDisposition::Handle
        );
        assert_eq!(engine.frame(), before);
        let exhausted = key(&mut engine, Key::Enter);
        assert!(!exhausted.handled);
        assert!(exhausted.commit.is_none());
        assert_eq!(exhausted.error.unwrap().code, ErrorCode::InvalidInput);
        assert_eq!(exhausted.frame, before);
    }
}
