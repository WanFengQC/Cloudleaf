//! 隔离输入宿主的翻译状态，不拥有候选或输入引擎。
use super::{CommitOutcome, LocalHost, TranslationReceipt};
use cloudleaf_input::TranslationSourceIdentity;
use cloudleaf_translation_integrity::{self as integrity, Glossary, IntegrityReport};
use cloudleaf_translation_ipc::{
    Direction, MAX_INPUT_BYTES, ProgressStage, RequestKey, ResourceThreads, SourceIdentity,
    WorkerClient, WorkerConfig, WorkerEvent, WorkerScheduling, WorkerState,
};
use std::{path::PathBuf, time::Duration};
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG};
use windows_core::{Error, Result};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(35);
pub(super) const COMMAND_FIRST: u16 = 21;
pub(super) const ENGLISH_EDIT: u16 = 29;
pub(super) const COMMAND_LAST: u16 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    StartChinese,
    StartEnglish,
    Cancel,
    Acknowledge,
    Confirm,
    KeepOriginal,
    SubmitEnglish,
    ClearEnglish,
    EnglishChanged,
    FocusChinese,
}

pub(super) fn command(wparam: usize, origin: bool) -> Option<Command> {
    if !origin {
        return None;
    }
    let (id, event) = (wparam as u16, (wparam >> 16) as u16);
    match (id, event) {
        (21, 0) => Some(Command::StartChinese),
        (22, 0) => Some(Command::StartEnglish),
        (23, 0) => Some(Command::Cancel),
        (24, 0) => Some(Command::Acknowledge),
        (25, 0) => Some(Command::Confirm),
        (26, 0) => Some(Command::KeepOriginal),
        (27, 0) => Some(Command::SubmitEnglish),
        (28, 0) => Some(Command::ClearEnglish),
        (COMMAND_LAST, 0) => Some(Command::FocusChinese),
        (ENGLISH_EDIT, 0x0300) => Some(Command::EnglishChanged),
        _ => None,
    }
}

pub(super) struct LaunchOptions<'a> {
    pub runtime: &'a str,
    pub runtime_sha: &'a str,
    pub model: Option<(&'a str, &'a str)>,
    worker: Option<WorkerConfig>,
    glossary: Option<PathBuf>,
}

enum LaunchThreads {
    Auto,
    Fixed(ResourceThreads),
}

impl<'a> LaunchOptions<'a> {
    pub fn parse(args: &'a [String]) -> Result<Self> {
        let usage = || {
            Error::new(
                E_INVALIDARG,
                "用法：--interactive-input-host RUNTIME SHA [MODEL MODEL_SHA] [--translation-catalog CATALOG SHA] [--translation-threads auto|1|2|4] [--translation-scheduling normal|below-normal] [--translation-glossary TSV]",
            )
        };
        let [runtime, runtime_sha, options @ ..] = args else {
            return Err(usage());
        };
        let mut remaining = options;
        let mut model = None;
        if remaining.first().is_some_and(|arg| !arg.starts_with("--")) {
            let [path, hash, rest @ ..] = remaining else {
                return Err(usage());
            };
            if hash.starts_with("--") {
                return Err(usage());
            }
            model = Some((path.as_str(), hash.as_str()));
            remaining = rest;
        }
        let mut catalog = None;
        let mut threads = None;
        let mut scheduling = None;
        let mut glossary = None;
        while !remaining.is_empty() {
            match remaining {
                [flag, path, hash, rest @ ..]
                    if flag == "--translation-catalog" && catalog.is_none() =>
                {
                    catalog = Some((PathBuf::from(path), hash.clone()));
                    remaining = rest;
                }
                [flag, value, rest @ ..]
                    if flag == "--translation-threads" && threads.is_none() =>
                {
                    threads = Some(if value == "auto" {
                        LaunchThreads::Auto
                    } else {
                        let value = value.parse::<usize>().map_err(|_| usage())?;
                        LaunchThreads::Fixed(ResourceThreads::try_from(value).map_err(|_| usage())?)
                    });
                    remaining = rest;
                }
                [flag, value, rest @ ..]
                    if flag == "--translation-scheduling" && scheduling.is_none() =>
                {
                    scheduling = Some(match value.as_str() {
                        "normal" => WorkerScheduling::Normal,
                        "below-normal" => WorkerScheduling::BelowNormal,
                        _ => return Err(usage()),
                    });
                    remaining = rest;
                }
                [flag, path, rest @ ..]
                    if flag == "--translation-glossary" && glossary.is_none() =>
                {
                    glossary = Some(PathBuf::from(path));
                    remaining = rest;
                }
                _ => return Err(usage()),
            }
        }
        if catalog.is_none() && (threads.is_some() || scheduling.is_some()) {
            return Err(usage());
        }
        let worker = catalog
            .map(|(path, hash)| {
                let config = match threads.unwrap_or(LaunchThreads::Fixed(ResourceThreads::One)) {
                    LaunchThreads::Auto => WorkerConfig::sibling_auto(path, hash),
                    LaunchThreads::Fixed(count) => WorkerConfig::sibling(path, hash, count),
                }?;
                config.with_scheduling(scheduling.unwrap_or_default())
            })
            .transpose()
            .map_err(|error| Error::new(E_INVALIDARG, format!("离线配置无效：{error}")))?;
        Ok(Self {
            runtime,
            runtime_sha,
            model,
            worker,
            glossary,
        })
    }

    pub fn controller(self) -> Result<Controller> {
        use std::io::Read as _;
        let glossary = if let Some(path) = self.glossary {
            let file = std::fs::File::open(path)
                .map_err(|_| Error::new(E_INVALIDARG, "无法读取指定的本地翻译术语表"))?;
            let mut data = String::new();
            file.take(integrity::MAX_GLOSSARY_BYTES as u64 + 1)
                .read_to_string(&mut data)
                .map_err(|_| Error::new(E_INVALIDARG, "本地翻译术语表不是有效 UTF-8"))?;
            Glossary::parse_tsv(&data)
                .map_err(|_| Error::new(E_INVALIDARG, "本地翻译术语表版本或规则无效"))?
        } else {
            Glossary::default()
        };
        Ok(Controller::new(self.worker, glossary))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Captured {
    pub context: SourceIdentity,
    pub source: TranslationSourceIdentity,
    pub direction: Direction,
    pub original: String,
}

impl Captured {
    pub fn new(local: &LocalHost, direction: Direction, english: &str) -> Result<Self> {
        if local.host.quarantined.get() {
            return Err(Error::new(E_FAIL, "请先核对未知提交并开始新会话"));
        }
        let engine = local.host.engine.borrow();
        let frame = engine.frame();
        let (candidate, original) = match direction {
            Direction::ZhEn => {
                let selected = frame
                    .highlighted
                    .and_then(|index| frame.candidates.get(index))
                    .ok_or_else(|| Error::new(E_INVALIDARG, "请先用方向键高亮当前中文候选"))?;
                (Some(selected.id.clone()), selected.text.as_str())
            }
            Direction::EnZh => {
                if !frame.preedit.is_empty() {
                    return Err(Error::new(
                        E_INVALIDARG,
                        "请先完成或取消中文组合，再处理英文缓冲区",
                    ));
                }
                (None, english)
            }
        };
        if original.trim().is_empty() || original.len() > MAX_INPUT_BYTES || original.contains('\0')
        {
            return Err(Error::new(E_INVALIDARG, "原文为空或超出允许长度"));
        }
        Ok(Self {
            context: SourceIdentity {
                session: local.host.instance,
                epoch: local.host.epoch.get(),
                input_revision: frame.revision,
            },
            source: engine.translation_source_identity(candidate),
            direction,
            original: original.to_owned(),
        })
    }

    pub fn current(&self, local: &LocalHost, english: &str, revision: u64) -> bool {
        if local.host.instance != self.context.session
            || local.host.epoch.get() != self.context.epoch
            || local.host.quarantined.get()
            || local.host.engine.borrow().revision() != revision
        {
            return false;
        }
        Self::new(local, self.direction, english)
            .is_ok_and(|current| current.original == self.original && current.source == self.source)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResultIdentity {
    pub key: RequestKey,
    task_id: u64,
    version: u32,
}

struct Preview {
    identity: ResultIdentity,
    raw: String,
    provider: String,
    model: String,
    report: IntegrityReport,
}

enum Phase {
    Pending(Option<u64>),
    Ready(Preview),
    Retry {
        preview: Preview,
        receipt: TranslationReceipt,
        revision: u64,
    },
    Unknown(Preview),
}

struct Active {
    capture: Captured,
    key: RequestKey,
    phase: Phase,
}

pub(super) struct PreparedConfirmation {
    pub identity: ResultIdentity,
    pub capture: Captured,
    pub text: String,
    pub retry: Option<TranslationReceipt>,
}

pub(super) struct View {
    pub status: String,
    pub source: String,
    pub output: String,
    pub confirm: bool,
    pub acknowledge: bool,
    pub retry: bool,
}

pub(super) struct Controller {
    worker: Option<WorkerClient>,
    glossary: Glossary,
    active: Option<Active>,
    acknowledged: Option<ResultIdentity>,
    focused: bool,
    status: String,
}

impl Controller {
    pub fn new(config: Option<WorkerConfig>, glossary: Glossary) -> Self {
        let configured = config.is_some();
        Self {
            worker: config.map(WorkerClient::new),
            glossary,
            active: None,
            acknowledged: None,
            focused: true,
            status: if configured {
                "离线模型将按需核验"
            } else {
                "未配置离线目录"
            }
            .into(),
        }
    }

    pub fn start(&mut self, local: &LocalHost, direction: Direction, english: &str) -> Result<()> {
        if !self.focused {
            return Err(Error::new(E_FAIL, "当前窗口没有可提交的输入焦点"));
        }
        if self.worker.is_none() {
            return Err(Error::new(
                E_FAIL,
                "未配置离线目录；请从命令行指定已核验的本地模型",
            ));
        }
        self.cancel(local, "旧请求已取消")?;
        let capture = Captured::new(local, direction, english)?;
        let key = self
            .worker
            .as_mut()
            .expect("已经核对离线配置")
            .submit(
                capture.context,
                direction,
                capture.original.clone(),
                REQUEST_TIMEOUT,
            )
            .map_err(|error| Error::new(E_FAIL, format!("无法开始离线翻译：{error}")))?;
        self.active = Some(Active {
            capture,
            key,
            phase: Phase::Pending(None),
        });
        self.status = "等待本地翻译工作进程".into();
        Ok(())
    }

    pub fn set_focus(&mut self, local: &LocalHost, focused: bool) -> Result<()> {
        self.focused = focused;
        if !focused {
            self.cancel(local, "焦点切换，旧翻译已失效")?;
        }
        Ok(())
    }

    pub fn cancel(&mut self, local: &LocalHost, message: &str) -> Result<()> {
        if let Some(active) = &self.active {
            match active.phase {
                Phase::Unknown(_) => return Ok(()),
                Phase::Retry { receipt, .. } => local.discard_translation(receipt)?,
                Phase::Pending(_) => {
                    if let Some(worker) = self.worker.as_mut() {
                        let _ = worker.cancel(active.key);
                    }
                }
                Phase::Ready(_) => (),
            }
            self.active = None;
            self.acknowledged = None;
            self.status = message.into();
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        if let Some(active) = self.active.take()
            && let Some(worker) = self.worker.as_mut()
        {
            let _ = worker.cancel(active.key);
        }
        self.acknowledged = None;
        self.status = "已开始新会话，旧译文不可提交".into();
    }

    pub fn invalidate_changed(
        &mut self,
        local: &LocalHost,
        english: Option<&str>,
        revision: u64,
    ) -> Result<bool> {
        let changed = self
            .active
            .as_ref()
            .is_some_and(|active| match active.phase {
                Phase::Unknown(_) => false,
                Phase::Retry {
                    revision: expected, ..
                } => {
                    revision != expected
                        || (active.capture.direction == Direction::EnZh
                            && english.is_some_and(|current| active.capture.original != current))
                }
                _ => {
                    revision != active.capture.context.input_revision
                        || (active.capture.direction == Direction::EnZh
                            && english.is_some_and(|current| active.capture.original != current))
                }
            });
        if changed {
            self.cancel(local, "输入已修改，旧译文不可提交")?;
        }
        Ok(changed)
    }

    pub fn poll(&mut self, local: &LocalHost, english: &str) -> bool {
        let mut changed = false;
        for _ in 0..16 {
            let Some(event) = self.worker.as_mut().and_then(|worker| worker.try_event()) else {
                break;
            };
            changed |= self.handle_event(local, english, event);
        }
        changed
    }

    fn handle_event(&mut self, local: &LocalHost, english: &str, event: WorkerEvent) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        let Phase::Pending(task) = &mut active.phase else {
            return false;
        };
        match event {
            WorkerEvent::State(WorkerState::Starting) => {
                self.status = "正在启动本地工作进程".into()
            }
            WorkerEvent::Accepted { key, task_id } if key == active.key && task.is_none() => {
                *task = Some(task_id);
                self.status = "本地工作进程已接受请求".into();
            }
            WorkerEvent::Progress {
                key,
                task_id,
                stage,
            } if key == active.key && *task == Some(task_id) => {
                self.status = match stage {
                    ProgressStage::Verifying => "正在核验离线模型",
                    ProgressStage::Loading => "正在加载所选方向的模型",
                    ProgressStage::Inferring => "正在本地翻译，可继续输入或取消",
                }
                .into();
            }
            WorkerEvent::Completed {
                key,
                task_id,
                result_version,
                output,
            } if key == active.key && *task == Some(task_id) && result_version != 0 => {
                if !self.focused
                    || !active
                        .capture
                        .current(local, english, key.source.input_revision)
                {
                    self.active = None;
                    self.status = "当前输入已变化，迟到译文已失效".into();
                    return true;
                }
                let direction = match active.capture.direction {
                    Direction::ZhEn => integrity::Direction::ChineseToEnglish,
                    Direction::EnZh => integrity::Direction::EnglishToChinese,
                };
                let mut report = integrity::check_translation(
                    &active.capture.original,
                    &output.raw_translation,
                    direction,
                    &self.glossary,
                );
                if output.metrics.output_truncated {
                    report.mark_truncated();
                }
                active.phase = Phase::Ready(Preview {
                    identity: ResultIdentity {
                        key,
                        task_id,
                        version: result_version,
                    },
                    raw: output.raw_translation,
                    provider: output.provider_identity,
                    model: output.model_identity,
                    report,
                });
                self.status = "译文就绪，等待明确确认".into();
            }
            WorkerEvent::Failed { key, error } if key == active.key => {
                self.status = format!("离线翻译失败：{error}。原文已保留。");
                self.active = None;
            }
            WorkerEvent::Cancelled { key } if key == active.key => {
                self.status = "翻译已取消，原文保留".into();
                self.active = None;
            }
            _ => return false,
        }
        true
    }

    pub fn acknowledge(&mut self) {
        if let Some(Active {
            phase: Phase::Ready(preview) | Phase::Retry { preview, .. },
            ..
        }) = &self.active
        {
            self.acknowledged = Some(preview.identity);
        }
    }

    pub fn prepare(&self, local: &LocalHost, english: &str) -> Result<PreparedConfirmation> {
        let active = self
            .active
            .as_ref()
            .ok_or_else(|| Error::new(E_FAIL, "当前没有可确认的译文"))?;
        let (preview, retry, revision) = match &active.phase {
            Phase::Ready(preview) => (preview, None, active.capture.context.input_revision),
            Phase::Retry {
                preview,
                receipt,
                revision,
            } => (preview, Some(*receipt), *revision),
            _ => return Err(Error::new(E_FAIL, "当前译文尚未就绪或提交状态未知")),
        };
        if !self.focused || !active.capture.current(local, english, revision) {
            return Err(Error::new(E_FAIL, "译文版本与当前输入不匹配"));
        }
        if preview.report.requires_acknowledgement() && self.acknowledged != Some(preview.identity)
        {
            return Err(Error::new(E_FAIL, "请先核对关键内容并明确确认当前风险版本"));
        }
        Ok(PreparedConfirmation {
            identity: preview.identity,
            capture: active.capture.clone(),
            text: preview.raw.clone(),
            retry,
        })
    }

    pub fn acknowledge_commit(
        &mut self,
        local: &LocalHost,
        confirmation: &PreparedConfirmation,
        receipt: TranslationReceipt,
    ) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let active_identity = match &active.phase {
            Phase::Ready(preview) | Phase::Retry { preview, .. } => preview.identity,
            _ => return,
        };
        if active_identity != confirmation.identity {
            return;
        }
        let phase = std::mem::replace(&mut active.phase, Phase::Pending(None));
        let preview = match phase {
            Phase::Ready(preview) | Phase::Retry { preview, .. } => preview,
            other => {
                active.phase = other;
                return;
            }
        };
        match receipt.outcome {
            CommitOutcome::Applied => {
                self.active = None;
                self.acknowledged = None;
                self.status = "已提交一次".into();
            }
            CommitOutcome::NotApplied => {
                active.phase = Phase::Retry {
                    preview,
                    receipt,
                    revision: local.host.engine.borrow().revision(),
                };
                self.status = "未提交，原文和译文已保留。可明确重试或保留原文。".into();
            }
            CommitOutcome::Unknown => {
                active.phase = Phase::Unknown(preview);
                self.acknowledged = None;
                self.status = "提交状态未知。请核对本宿主文档后开始新会话。".into();
            }
        }
    }

    pub fn direction(&self) -> Option<Direction> {
        self.active.as_ref().map(|active| active.capture.direction)
    }

    pub fn set_message(&mut self, message: &str) {
        self.status = message.into();
    }

    pub fn report_error(&mut self, error: &Error) {
        self.status = error.message().to_string();
    }

    pub fn view(&self) -> View {
        let active = self.active.as_ref();
        let phase = active.map(|active| &active.phase);
        let preview = match phase {
            Some(
                Phase::Ready(preview) | Phase::Retry { preview, .. } | Phase::Unknown(preview),
            ) => Some(preview),
            _ => None,
        };
        let mut status = self.status.clone();
        let needs_ack = preview.is_some_and(|p| {
            p.report.requires_acknowledgement() && self.acknowledged != Some(p.identity)
        });
        if let Some(preview) = preview {
            let findings = preview
                .report
                .findings
                .iter()
                .filter(|item| item.status.requires_acknowledgement());
            let count = findings.clone().count();
            if count != 0 {
                let labels = findings
                    .take(4)
                    .map(|item| format!("{}：{}", item.kind.label(), item.status.label()))
                    .collect::<Vec<_>>()
                    .join("、");
                status.push_str(&format!("  共 {count} 项需核对：{labels}"));
                if count > 4 {
                    status.push_str(&format!("等（另 {} 项）", count - 4));
                }
            }
            const IDENTITY_CHARS: usize = 12;
            let identity = preview
                .model
                .chars()
                .take(IDENTITY_CHARS)
                .collect::<String>();
            status.push_str(&format!("  {} · {} · 研发模型", preview.provider, identity));
        }
        let ready = matches!(phase, Some(Phase::Ready(_) | Phase::Retry { .. }));
        View {
            status,
            source: active.map_or(String::new(), |active| active.capture.original.clone()),
            output: preview.map_or(String::new(), |preview| preview.raw.clone()),
            confirm: ready && !needs_ack && self.focused,
            acknowledge: ready && needs_ack && self.focused,
            retry: matches!(phase, Some(Phase::Retry { .. })),
        }
    }

    pub fn close(&mut self) {
        if let Some(worker) = self.worker.as_mut() {
            worker.close();
        }
    }

    pub fn wait(&mut self) -> bool {
        self.worker
            .as_mut()
            .is_none_or(|worker| worker.wait_for_shutdown(Duration::from_secs(3)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudleaf_translation_ipc::{QualityStatus, WorkerMetrics, WorkerOutput};
    use cloudleaf_types::{InputEvent, Key};

    fn host() -> LocalHost {
        LocalHost::new(super::super::Host::new().unwrap()).unwrap()
    }

    fn pending(
        controller: &mut Controller,
        local: &LocalHost,
        english: &str,
        request_id: u64,
    ) -> RequestKey {
        let capture = Captured::new(local, Direction::EnZh, english).unwrap();
        let key = RequestKey {
            generation: 1,
            source: capture.context,
            request_id,
        };
        controller.active = Some(Active {
            capture,
            key,
            phase: Phase::Pending(None),
        });
        key
    }

    fn result(key: RequestKey, version: u32, text: &str) -> WorkerEvent {
        WorkerEvent::Completed {
            key,
            task_id: 10,
            result_version: version,
            output: WorkerOutput {
                raw_translation: text.into(),
                direction: Direction::EnZh,
                provider_identity: "offline-model".into(),
                model_identity: "b".repeat(64),
                catalog_sha256: "a".repeat(64),
                quality: QualityStatus::ResearchOnly,
                metrics: Box::new(WorkerMetrics {
                    compute_threads: 2,
                    resident_models: 1,
                    verification_started_unix_ns: 1,
                    loading_started_unix_ns: 2,
                    inference_started_unix_ns: 3,
                    inference_finished_unix_ns: 4,
                    verification_ns: 1,
                    load_ns: 1,
                    inference_ns: 1,
                    encoder_ns: 1,
                    decoder_ns: 1,
                    generated_steps: 1,
                    queue_wait_ns: 0,
                    output_truncated: false,
                }),
            },
        }
    }

    #[test]
    #[ignore = "需要本地固定 Marian 和相邻 Worker；无窗口或系统操作"]
    fn round26_isolated_host_uses_real_worker_in_both_directions() {
        let catalog = PathBuf::from(std::env::var_os("CLOUDLEAF_ROUND26_CATALOG").unwrap());
        let digest = std::env::var("CLOUDLEAF_ROUND26_CATALOG_SHA").unwrap();
        let executable = PathBuf::from(std::env::var_os("CLOUDLEAF_ROUND26_HOST").unwrap());
        let config = WorkerConfig::sibling(catalog, digest, ResourceThreads::One)
            .unwrap()
            .for_host_executable(executable)
            .unwrap();
        let local = host();
        let mut controller = Controller::new(Some(config), Glossary::default());

        for (direction, english, model) in [
            (
                Direction::EnZh,
                "Please close the window.",
                "opus-mt-en-zh@408d9bc410a388e1d9aef112a2daba955b945255",
            ),
            (
                Direction::ZhEn,
                "",
                "opus-mt-zh-en@cf109095479db38d6df799875e34039d4938aaa6",
            ),
        ] {
            if direction == Direction::ZhEn {
                for key in "nihao".chars().map(Key::Char).chain([Key::Down]) {
                    local
                        .process(InputEvent::Key(key), None, &|_| panic!("候选预览不提交"))
                        .unwrap();
                }
            }
            controller.start(&local, direction, english).unwrap();
            let until = std::time::Instant::now() + REQUEST_TIMEOUT;
            loop {
                controller.poll(&local, english);
                match controller.active.as_ref().map(|active| &active.phase) {
                    Some(Phase::Ready(preview)) => {
                        assert!(preview.identity.task_id > 0 && preview.identity.version > 0);
                        assert!(!preview.raw.trim().is_empty());
                        assert_ne!(preview.raw, controller.view().source);
                        assert_eq!(preview.raw, controller.view().output);
                        assert_eq!(preview.model, model);
                        println!(
                            "isolated-host direction={direction:?} model={} bytes={}",
                            preview.model,
                            preview.raw.len()
                        );
                        break;
                    }
                    Some(Phase::Pending(_)) if std::time::Instant::now() < until => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    _ => panic!("真实 Worker 未完成：{}", controller.view().status),
                }
            }
            assert!(local.document.borrow().is_empty());
            controller.cancel(&local, "测试结束").unwrap();
            assert!(controller.view().output.is_empty());
        }
        local
            .process(InputEvent::Key(Key::Escape), None, &|_| {
                panic!("Esc 不提交")
            })
            .unwrap();
        controller
            .start(&local, Direction::EnZh, "Please try again.")
            .unwrap();
        controller.cancel(&local, "已取消").unwrap();
        local
            .process(InputEvent::Key(Key::Char('a')), None, &|_| {
                panic!("按键不提交")
            })
            .unwrap();
        assert_eq!(local.host.engine.borrow().frame().preedit, "a");
        controller.close();
        assert!(controller.wait());
    }
    #[test]
    fn round22_argument_parser_retains_input_model_and_rejects_stray_config() {
        let legacy = ["runtime", "sha", "model", "model-sha"].map(|value| value.to_owned());
        let options = LaunchOptions::parse(&legacy).unwrap();
        assert_eq!(options.model, Some(("model", "model-sha")));
        assert!(options.worker.is_none());

        let configured = [
            "runtime",
            "sha",
            "--translation-catalog",
            "catalog",
            &"a".repeat(64),
            "--translation-threads",
            "1",
            "--translation-glossary",
            "glossary",
        ]
        .map(|value| value.to_owned());
        let options = LaunchOptions::parse(&configured).unwrap();
        assert_eq!(options.model, None);
        assert!(options.worker.is_some());
        assert_eq!(options.glossary, Some(PathBuf::from("glossary")));
        let stray = ["runtime", "sha", "--translation-threads", "4"].map(|value| value.to_owned());
        assert!(LaunchOptions::parse(&stray).is_err());
    }

    #[test]
    fn round25_argument_parser_offers_optional_worker_resource_control() {
        let catalog = "a".repeat(64);
        let configured = [
            "runtime",
            "sha",
            "--translation-catalog",
            "catalog",
            &catalog,
            "--translation-threads",
            "auto",
            "--translation-scheduling",
            "below-normal",
        ]
        .map(str::to_owned);
        let worker = LaunchOptions::parse(&configured).unwrap().worker.unwrap();
        let available = std::thread::available_parallelism().map_or(1, usize::from);
        assert_eq!(
            worker.compute_threads(),
            ResourceThreads::for_available_parallelism(available)
        );
        let mut invalid = configured.clone();
        invalid[8] = "high".to_owned();
        assert!(LaunchOptions::parse(&invalid).is_err());
        let stray = ["runtime", "sha", "--translation-scheduling", "normal"].map(str::to_owned);
        assert!(LaunchOptions::parse(&stray).is_err());
        let mut explicit = configured;
        explicit[6] = "4".to_owned();
        explicit[8] = "normal".to_owned();
        assert_eq!(
            LaunchOptions::parse(&explicit)
                .unwrap()
                .worker
                .unwrap()
                .compute_threads(),
            ResourceThreads::Four
        );
    }

    #[test]
    fn round22_command_mapping_requires_matching_origin_and_notification() {
        assert_eq!(command(21, true), Some(Command::StartChinese));
        assert_eq!(command(22, true), Some(Command::StartEnglish));
        assert_eq!(command(25, true), Some(Command::Confirm));
        assert_eq!(command(25, false), None);
        assert_eq!(command(25 | (0x0300 << 16), true), None);
        assert_eq!(
            command(usize::from(ENGLISH_EDIT) | (0x0300 << 16), true),
            Some(Command::EnglishChanged)
        );
        assert_eq!(command(usize::from(ENGLISH_EDIT), true), None);
        assert_eq!(command(4, true), None);
    }

    #[test]
    fn round22_risk_ack_is_bound_to_request_task_and_exact_result_version() {
        let local = host();
        let mut controller = Controller::new(None, Glossary::default());
        let original = "USD 13.45";
        let key = pending(&mut controller, &local, original, 4);
        assert!(!controller.handle_event(&local, original, result(key, 1, "13.45元")));
        assert!(controller.handle_event(
            &local,
            original,
            WorkerEvent::Accepted { key, task_id: 10 }
        ));
        assert!(controller.handle_event(&local, original, result(key, 1, "13.45元")));
        assert!(controller.view().acknowledge);
        assert!(controller.prepare(&local, original).is_err());
        controller.acknowledge();
        assert_eq!(
            controller.prepare(&local, original).unwrap().text,
            "13.45元"
        );
        assert!(!controller.handle_event(&local, original, result(key, 2, "13.45欧元")));

        controller.cancel(&local, "重新翻译").unwrap();
        let key = pending(&mut controller, &local, original, 5);
        assert!(controller.handle_event(
            &local,
            original,
            WorkerEvent::Accepted { key, task_id: 10 }
        ));
        assert!(controller.handle_event(&local, original, result(key, 2, "13.45欧元")));
        assert!(controller.prepare(&local, original).is_err());
        assert!(controller.view().acknowledge);
    }

    #[test]
    fn round22_escape_retires_known_failed_preview_after_dual_core_clear() {
        let local = host();
        let mut controller = Controller::new(None, Glossary::default());
        let original = "Hello.";
        let key = pending(&mut controller, &local, original, 1);
        assert!(controller.handle_event(
            &local,
            original,
            WorkerEvent::Accepted { key, task_id: 10 }
        ));
        assert!(controller.handle_event(&local, original, result(key, 1, "你好。")));
        let confirmation = controller.prepare(&local, original).unwrap();
        let receipt = local
            .translation_commit(&confirmation.capture, &confirmation.text, None, &|_| {
                CommitOutcome::NotApplied
            })
            .unwrap();
        controller.acknowledge_commit(&local, &confirmation, receipt);
        assert!(controller.view().retry);

        let (frame, _) = local
            .process(InputEvent::Key(Key::Escape), None, &|_| {
                panic!("Esc 不提交")
            })
            .unwrap();
        assert!(
            controller
                .invalidate_changed(&local, Some(original), frame.revision)
                .unwrap()
        );
        assert!(!controller.view().retry);
        assert!(controller.view().output.is_empty());
        assert!(local.document.borrow().is_empty());

        let key = pending(&mut controller, &local, original, 2);
        assert!(controller.handle_event(
            &local,
            original,
            WorkerEvent::Accepted { key, task_id: 10 }
        ));
        assert!(controller.handle_event(&local, original, result(key, 1, "你好。")));
        let confirmation = controller.prepare(&local, original).unwrap();
        let receipt = local
            .translation_commit(&confirmation.capture, &confirmation.text, None, &|_| {
                CommitOutcome::Applied
            })
            .unwrap();
        assert_eq!(receipt.outcome, CommitOutcome::Applied);
        assert_eq!(&*local.document.borrow(), "你好。");
    }

    #[test]
    fn round22_one_sided_clear_keeps_failed_preview_and_blocks_retry() {
        let local = host();
        let mut controller = Controller::new(None, Glossary::default());
        let original = "Hello.";
        let key = pending(&mut controller, &local, original, 1);
        assert!(controller.handle_event(
            &local,
            original,
            WorkerEvent::Accepted { key, task_id: 10 }
        ));
        assert!(controller.handle_event(&local, original, result(key, 1, "你好。")));
        let confirmation = controller.prepare(&local, original).unwrap();
        let receipt = local
            .translation_commit(&confirmation.capture, &confirmation.text, None, &|_| {
                CommitOutcome::NotApplied
            })
            .unwrap();
        controller.acknowledge_commit(&local, &confirmation, receipt);

        local
            .host
            .projection
            .borrow_mut()
            .handle(InputEvent::Key(Key::Escape));
        assert!(
            controller
                .cancel(&local, "只清除投影时不能丢失预览")
                .is_err()
        );
        assert!(controller.view().retry);
        assert_eq!(controller.view().output, "你好。");
        assert!(
            local
                .translation_commit(
                    &confirmation.capture,
                    &confirmation.text,
                    Some(receipt),
                    &|_| panic!("失配不得提交"),
                )
                .is_err()
        );
        assert!(local.document.borrow().is_empty());
    }

    #[test]
    fn round22_risk_summary_reports_omitted_warning_count() {
        let local = host();
        let mut controller = Controller::new(None, Glossary::default());
        let original = "1 2 3 4 5 6";
        let key = pending(&mut controller, &local, original, 1);
        assert!(controller.handle_event(
            &local,
            original,
            WorkerEvent::Accepted { key, task_id: 10 }
        ));
        assert!(controller.handle_event(&local, original, result(key, 1, "数字缺失")));
        assert!(controller.view().status.contains("共 6 项"));
        assert!(controller.view().status.contains("另 2 项"));
        assert!(controller.prepare(&local, original).is_err());
    }

    #[test]
    fn round22_edit_and_focus_invalidate_owned_translation_without_touching_input() {
        let local = host();
        let mut controller = Controller::new(None, Glossary::default());
        let key = pending(&mut controller, &local, "Hello.", 1);
        assert!(controller.handle_event(
            &local,
            "Hello.",
            WorkerEvent::Accepted { key, task_id: 10 }
        ));
        assert!(
            controller
                .invalidate_changed(
                    &local,
                    Some("Edited."),
                    local.host.engine.borrow().revision()
                )
                .unwrap()
        );
        assert!(!controller.handle_event(&local, "Hello.", result(key, 1, "你好。")));
        assert!(controller.prepare(&local, "Hello.").is_err());

        let key = pending(&mut controller, &local, "Hello.", 2);
        assert!(controller.handle_event(
            &local,
            "Hello.",
            WorkerEvent::Accepted { key, task_id: 10 }
        ));
        local
            .process(InputEvent::Key(Key::Char('n')), None, &|_| {
                panic!("未提交输入")
            })
            .unwrap();
        assert!(controller.handle_event(&local, "Hello.", result(key, 1, "你好。")));
        assert!(!controller.view().confirm);
        assert_eq!(local.host.engine.borrow().frame().preedit, "n");

        local
            .process(InputEvent::FocusLost, None, &|_| panic!("失焦不提交"))
            .unwrap();
        let key = pending(&mut controller, &local, "Hello.", 3);
        controller.set_focus(&local, false).unwrap();
        assert!(!controller.handle_event(&local, "Hello.", result(key, 1, "你好。")));
        assert!(controller.prepare(&local, "Hello.").is_err());
    }
}
