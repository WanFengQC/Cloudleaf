//! 已接管原生消息的有界所有权材料；不替代核心算法，不自动重发原生命令。
use cloudleaf_types::{EventResult, Key};
use std::{collections::VecDeque, time::Instant};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows_core::{Error, Result};

pub(crate) const OWNED_EVENT_LIMIT: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    Queued,
    Started,
    Writing,
    Applied,
    NotApplied,
    Unknown,
}
#[derive(Debug, Clone)]
pub(crate) struct NativeEnvelope {
    pub instance: u64,
    pub epoch: u64,
    pub sequence: u64,
    pub wparam: usize,
    pub lparam: isize,
    pub layout: usize,
    pub modifiers: u8,
    pub key: Key,
    pub repeat: u16,
}
impl NativeEnvelope {
    pub fn capture(
        instance: u64,
        epoch: u64,
        sequence: u64,
        key: Key,
        w: WPARAM,
        l: LPARAM,
    ) -> Self {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        // 安全：只捕获当前线程键盘元数据，不保存系统指针或用户文档内容。
        let (layout, modifiers) = unsafe {
            let flags = [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
                .iter()
                .enumerate()
                .fold(0, |bits, (index, key)| {
                    bits | (u8::from(GetKeyState(i32::from(key.0)) < 0) << index)
                });
            (GetKeyboardLayout(0).0 as usize, flags)
        };
        Self {
            instance,
            epoch,
            sequence,
            wparam: w.0,
            lparam: l.0,
            layout,
            modifiers,
            key,
            repeat: (l.0 as usize & 0xffff) as u16,
        }
    }
}
#[derive(Clone)]
pub(crate) struct OwnedEvent {
    pub native: NativeEnvelope,
    pub planned: EventResult,
    pub outcome: Outcome,
    pub admitted: Instant,
}

/// 只保留当前尚未完成的接管材料；已应用事件不会累积为输入历史。
pub(crate) struct Ledger {
    events: VecDeque<OwnedEvent>,
    instance: Option<u64>,
    epoch: Option<u64>,
    last_sequence: u64,
    isolated: bool,
}

fn invalid_event() -> Error {
    Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG)
}

impl Ledger {
    pub fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(OWNED_EVENT_LIMIT),
            instance: None,
            epoch: None,
            last_sequence: 0,
            isolated: false,
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    pub fn front(&self) -> Option<&OwnedEvent> {
        self.events.front()
    }

    /// ownership 决策前验证全部原生身份条件；不改变水位或投影状态。
    pub fn can_accept(&self, native: &NativeEnvelope) -> bool {
        !self.isolated
            && self.events.len() < OWNED_EVENT_LIMIT
            && native.instance != 0
            && native.sequence > self.last_sequence
            && native.repeat == 1
            && (native.lparam as usize & 0xffff) == usize::from(native.repeat)
            && native.modifiers & !1 == 0
            && self
                .instance
                .is_none_or(|instance| instance == native.instance)
            && self.epoch.is_none_or(|epoch| epoch <= native.epoch)
            && (self.events.is_empty() || self.epoch == Some(native.epoch))
    }

    /// 只有已决定接管的 IME 事件才能入队；拒绝不会更改现有材料或消耗序号。
    pub fn push(&mut self, event: OwnedEvent) -> Result<()> {
        let native = &event.native;
        if !self.can_accept(native) {
            return Err(Error::from_hresult(windows::Win32::Foundation::E_FAIL));
        }
        if event.outcome != Outcome::Queued
            || !event.planned.handled
            || event.planned.error.is_some()
        {
            return Err(invalid_event());
        }
        self.instance = Some(native.instance);
        self.epoch = Some(native.epoch);
        self.last_sequence = native.sequence;
        self.events.push_back(event);
        Ok(())
    }

    fn head_mut(&mut self, sequence: u64) -> Result<&mut OwnedEvent> {
        self.events
            .front_mut()
            .filter(|event| event.native.sequence == sequence)
            .ok_or_else(invalid_event)
    }

    /// 出队执行只允许当前队首；克隆后的值不跨 COM 外调持有可变借用。
    pub fn start(&mut self, sequence: u64) -> Result<OwnedEvent> {
        if self.isolated {
            return Err(invalid_event());
        }
        let event = self.head_mut(sequence)?;
        if event.outcome != Outcome::Queued {
            return Err(invalid_event());
        }
        event.outcome = Outcome::Started;
        Ok(event.clone())
    }

    /// 第一次可能修改原文的外调之前调用，之后失败不能再证明未应用。
    pub fn writing(&mut self, sequence: u64) -> Result<()> {
        if self.isolated {
            return Err(invalid_event());
        }
        let event = self.head_mut(sequence)?;
        if event.outcome != Outcome::Started {
            return Err(invalid_event());
        }
        event.outcome = Outcome::Writing;
        Ok(())
    }

    /// 调用方须先证明真实编辑完成；序号失配、重复确认或隔离均拒绝。
    pub fn applied(&mut self, sequence: u64) -> Result<()> {
        if self.isolated {
            return Err(invalid_event());
        }
        let event = self.head_mut(sequence)?;
        if !matches!(event.outcome, Outcome::Started | Outcome::Writing) {
            return Err(invalid_event());
        }
        event.outcome = Outcome::Applied;
        self.events.pop_front();
        Ok(())
    }

    /// 失败只增加确定性限制，不允许把可能写入的事件降为可重试失败。
    pub fn fail(&mut self, sequence: u64, outcome: Outcome) -> Result<()> {
        let event = self.head_mut(sequence)?;
        let valid = match outcome {
            Outcome::NotApplied => matches!(
                event.outcome,
                Outcome::Queued | Outcome::Started | Outcome::NotApplied
            ),
            Outcome::Unknown => matches!(
                event.outcome,
                Outcome::Started | Outcome::Writing | Outcome::Unknown
            ),
            _ => false,
        };
        if !valid {
            return Err(invalid_event());
        }
        event.outcome = outcome;
        self.isolate();
        Ok(())
    }

    /// 排队和尚未写入的事件保留为未应用，进入写入的事件保留为未知。
    pub fn isolate(&mut self) {
        self.isolated = true;
        for event in &mut self.events {
            event.outcome = match event.outcome {
                Outcome::Queued | Outcome::Started => Outcome::NotApplied,
                Outcome::Writing => Outcome::Unknown,
                outcome => outcome,
            };
        }
    }

    pub fn snapshot(&self) -> Vec<OwnedEvent> {
        self.events.iter().cloned().collect()
    }

    /// 仅用于用户明确放弃或已移交恢复窗口的材料；身份水位仍保留以拒绝回流。
    pub fn clear(&mut self) {
        self.events.clear();
        self.isolated = false;
    }
}
pub(crate) fn recovery_material(reason: &str, events: &[OwnedEvent]) -> String {
    let mut result = format!(
        "云笺未完成输入恢复材料\r\n原因：{reason}\r\n原生命令没有重放；Unknown可能已经写入，不能自动重试。\r\n"
    );
    for event in events {
        let n = &event.native;
        result.push_str(&format!("\r\ninstance={} epoch={} sequence={} state={:?} repeat={} age_ms={}\r\n原始键={:?} VK={} LPARAM={:X} layout={:X} modifiers={:X}\r\n", n.instance, n.epoch, n.sequence, event.outcome, n.repeat, event.admitted.elapsed().as_millis(), n.key, n.wparam, n.lparam, n.layout, n.modifiers));
        if matches!(
            event.outcome,
            Outcome::Queued | Outcome::Started | Outcome::NotApplied
        ) {
            if let Some(commit) = &event.planned.commit {
                result.push_str(&format!("计划提交（未确认应用）：{}\r\n", commit.text));
            } else {
                result.push_str(&format!(
                    "计划预编辑（未确认应用）：{}\r\n",
                    event.planned.frame.preedit
                ));
            }
        } else {
            result
                .push_str("已经进入写入或结果未知：仅供检查原始事件，禁止直接当成未应用提交。\r\n");
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudleaf_dictionary::Dictionary;
    use cloudleaf_input::InputEngine;
    use cloudleaf_types::InputEvent;

    fn owned(sequence: u64, key: Key) -> OwnedEvent {
        let mut engine = InputEngine::new(Dictionary::empty());
        engine.handle(InputEvent::Activate);
        let planned = engine.handle(InputEvent::Key(key));
        OwnedEvent {
            native: NativeEnvelope {
                instance: 7,
                epoch: 2,
                sequence,
                wparam: 0x4e,
                lparam: 1,
                layout: 0,
                modifiers: 0,
                key,
                repeat: 1,
            },
            planned,
            outcome: Outcome::Queued,
            admitted: Instant::now(),
        }
    }

    #[test]
    fn fifo_preserves_distinct_identical_native_messages_and_rejects_duplicate_callbacks() {
        let mut ledger = Ledger::new();
        ledger.push(owned(1, Key::Char('n'))).unwrap();
        ledger.push(owned(2, Key::Char('n'))).unwrap();
        assert!(ledger.start(2).is_err());
        assert_eq!(ledger.start(1).unwrap().native.sequence, 1);
        assert!(ledger.start(1).is_err());
        ledger.writing(1).unwrap();
        assert!(ledger.writing(1).is_err());
        ledger.applied(1).unwrap();
        assert!(ledger.applied(1).is_err());
        assert!(ledger.push(owned(1, Key::Char('n'))).is_err());
        assert_eq!(ledger.front().unwrap().native.sequence, 2);
        ledger.start(2).unwrap();
        ledger.applied(2).unwrap();
        assert!(ledger.is_empty());
    }

    #[test]
    fn queue_full_rejects_new_ownership_without_overwriting_or_advancing_watermark() {
        let mut ledger = Ledger::new();
        for sequence in 1..=OWNED_EVENT_LIMIT as u64 {
            ledger.push(owned(sequence, Key::Char('n'))).unwrap();
        }
        let next = OWNED_EVENT_LIMIT as u64 + 1;
        assert!(ledger.push(owned(next, Key::Char('i'))).is_err());
        assert_eq!(ledger.len(), OWNED_EVENT_LIMIT);
        assert_eq!(ledger.snapshot().last().unwrap().native.sequence, next - 1);
        ledger.start(1).unwrap();
        ledger.applied(1).unwrap();
        ledger.push(owned(next, Key::Char('i'))).unwrap();
        assert_eq!(ledger.snapshot().last().unwrap().native.key, Key::Char('i'));
    }

    #[test]
    fn writing_and_unknown_never_become_not_applied_or_automatically_restart() {
        let mut ledger = Ledger::new();
        ledger.push(owned(1, Key::Char('n'))).unwrap();
        ledger.push(owned(2, Key::Char('i'))).unwrap();
        ledger.start(1).unwrap();
        ledger.writing(1).unwrap();
        assert!(ledger.fail(1, Outcome::NotApplied).is_err());
        assert_eq!(ledger.front().unwrap().outcome, Outcome::Writing);
        ledger.fail(1, Outcome::Unknown).unwrap();
        assert_eq!(
            ledger
                .snapshot()
                .iter()
                .map(|e| e.outcome)
                .collect::<Vec<_>>(),
            [Outcome::Unknown, Outcome::NotApplied]
        );
        assert!(ledger.start(1).is_err());
        assert!(ledger.applied(1).is_err());
        assert!(ledger.fail(1, Outcome::NotApplied).is_err());
        assert!(ledger.push(owned(3, Key::Char('a'))).is_err());
        ledger.clear();
        assert!(ledger.push(owned(2, Key::Char('i'))).is_err());
        ledger.push(owned(3, Key::Char('a'))).unwrap();
    }

    #[test]
    fn isolate_preserves_identity_order_raw_repeat_and_known_unwritten_material() {
        let mut ledger = Ledger::new();
        ledger.push(owned(1, Key::Char('n'))).unwrap();
        let mut different = owned(2, Key::Char('i'));
        different.native.epoch += 1;
        assert!(ledger.push(different).is_err());
        ledger.push(owned(2, Key::Char('i'))).unwrap();
        ledger.start(1).unwrap();
        ledger.isolate();
        assert_eq!(
            ledger
                .snapshot()
                .iter()
                .map(|e| e.native.sequence)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(
            ledger
                .snapshot()
                .iter()
                .all(|e| e.outcome == Outcome::NotApplied)
        );
        ledger.clear();
        let mut stale = owned(3, Key::Char('a'));
        stale.native.epoch = 1;
        assert!(ledger.push(stale).is_err());
        let mut other = owned(3, Key::Char('a'));
        other.native.instance = 8;
        assert!(ledger.push(other).is_err());
        let mut current = owned(3, Key::Char('a'));
        current.native.epoch = 3;
        ledger.push(current).unwrap();
    }

    #[test]
    fn passthrough_shortcut_merged_repeat_and_core_error_are_not_owned() {
        let mut ledger = Ledger::new();
        assert!(ledger.push(owned(1, Key::Space)).is_err());
        let mut shortcut = owned(1, Key::Char('n'));
        shortcut.native.modifiers = 2;
        assert!(ledger.push(shortcut).is_err());
        let mut repeat = owned(1, Key::Char('n'));
        repeat.native.repeat = 2;
        repeat.native.lparam = 2;
        assert!(ledger.push(repeat).is_err());
        assert!(ledger.push(owned(1, Key::Char('\0'))).is_err());
        assert!(ledger.is_empty());
        ledger.push(owned(1, Key::Char('n'))).unwrap();
    }

    #[test]
    fn uncertain_recovery_does_not_label_planned_commit_as_unapplied() {
        let mut event = owned(1, Key::Char('n'));
        event.planned.commit = Some(cloudleaf_types::CommitRequest {
            id: 1,
            text: "唯一不确定文本".into(),
        });
        for outcome in [Outcome::Writing, Outcome::Unknown] {
            event.outcome = outcome;
            let material = recovery_material("故障注入", &[event.clone()]);
            assert!(material.contains("sequence=1"));
            assert!(material.contains("结果未知"));
            assert!(!material.contains("唯一不确定文本"));
        }
        event.outcome = Outcome::NotApplied;
        assert!(recovery_material("写入前失败", &[event]).contains("唯一不确定文本"));
    }
}
