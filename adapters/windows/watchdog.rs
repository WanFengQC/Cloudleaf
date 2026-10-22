//! 独立超时检测只持有原子信号，绝不持有或调用 apartment COM。
use super::service::Lease;
use std::{
    os::windows::io::AsRawHandle,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicIsize, AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM},
    System::Threading::WaitForSingleObject,
    UI::WindowsAndMessaging::{PostMessageW, WM_APP},
};

pub(crate) const NOTICE: u32 = WM_APP + 0x3a1;
const INTERVAL: Duration = Duration::from_millis(10);
const HEARTBEAT_LIMIT: u64 = 250;
const QUEUED: u64 = 1;
const STARTED: u64 = 2;
const WRITING: u64 = 3;
const EXPIRED: u64 = 4;
const UNKNOWN: u64 = 5;
const EXPIRED_STARTED: u64 = 6;
const MASK: u64 = 15;
static CLOCK: OnceLock<Instant> = OnceLock::new();
static RETIRED: OnceLock<Mutex<Vec<Retired>>> = OnceLock::new();
static SLOTS: AtomicUsize = AtomicUsize::new(0);
const SLOT_LIMIT: usize = 32;
const NOTICE_ATTEMPTS: u8 = 4;
#[cfg(test)]
thread_local! { static SPAWN_FAILURE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }
fn millis() -> u64 {
    CLOCK.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

pub(crate) struct Signals {
    token: AtomicU64,
    deadline: AtomicU64,
    primary: AtomicU64,
    observer: AtomicU64,
    failed: AtomicU8,
    stop: AtomicBool,
    window: AtomicIsize,
    delivery_failed: AtomicBool,
    notice_pending: AtomicBool,
    notice_attempts: AtomicU8,
    #[cfg(test)]
    fail_delivery: AtomicBool,
    #[cfg(test)]
    pause_primary: AtomicBool,
}
pub(crate) struct Watchdog {
    signals: Arc<Signals>,
    retired: Option<Retired>,
}
struct Retired {
    workers: Vec<JoinHandle<()>>,
    _lease: Lease,
    finished: Arc<AtomicBool>,
}
impl Drop for Retired {
    fn drop(&mut self) {
        SLOTS.fetch_sub(1, Ordering::AcqRel);
    }
}
pub(crate) fn reap() {
    let Some(retired) = RETIRED.get() else {
        return;
    };
    let Ok(mut tasks) = retired.lock() else {
        return;
    };
    let mut index = 0;
    while index < tasks.len() {
        let exited = tasks[index].workers.iter().all(|worker| {
            // 安全：JoinHandle 持有真实线程句柄；只零等待，不能阻塞调用线程。
            unsafe { WaitForSingleObject(HANDLE(worker.as_raw_handle()), 0) == WAIT_OBJECT_0 }
        });
        if exited {
            let mut task = tasks.swap_remove(index);
            for worker in task.workers.drain(..) {
                let _ = worker.join();
            }
            task.finished.store(true, Ordering::Release);
            drop(task);
        } else {
            index += 1;
        }
    }
}
fn retire(task: Retired) {
    // 每次启动前已预留一个 slot，因此集合容量不可能超过上限；中毒时保留 lease 而不冒险卸载。
    match RETIRED
        .get_or_init(|| Mutex::new(Vec::with_capacity(SLOT_LIMIT)))
        .lock()
    {
        Ok(mut tasks) => tasks.push(task),
        Err(_) => std::mem::forget(task),
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Detection {
    Healthy,
    QueuedExpired,
    StartedExpired,
    Unknown,
    MonitorFailed,
}

impl Signals {
    fn notify(&self) {
        let window = self.window.load(Ordering::Acquire);
        if window != 0 {
            if self
                .notice_pending
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return;
            }
            if self
                .notice_attempts
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |attempts| {
                    (attempts < NOTICE_ATTEMPTS).then_some(attempts + 1)
                })
                .is_err()
            {
                self.notice_pending.store(false, Ordering::Release);
                return;
            }
            // 安全：只发无文本通知；STA 自己识别当前状态，不跨线程触及窗口数据或 COM。
            #[cfg(test)]
            if self.fail_delivery.load(Ordering::Acquire) {
                self.delivery_failed.store(true, Ordering::Release);
                self.notice_pending.store(false, Ordering::Release);
                return;
            }
            let sent =
                unsafe { PostMessageW(Some(HWND(window as *mut _)), NOTICE, WPARAM(0), LPARAM(0)) };
            if sent.is_err() {
                self.delivery_failed.store(true, Ordering::Release);
                self.notice_pending.store(false, Ordering::Release);
            }
        }
    }
    fn tick(&self, primary: bool) {
        let now = millis();
        let (own, peer) = if primary {
            (&self.primary, &self.observer)
        } else {
            (&self.observer, &self.primary)
        };
        own.store(now, Ordering::Release);
        if now.saturating_sub(peer.load(Ordering::Acquire)) > HEARTBEAT_LIMIT
            && self
                .failed
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            self.notify();
        }
        let token = self.token.load(Ordering::Acquire);
        let phase = token & MASK;
        let deadline = self.deadline.load(Ordering::Acquire);
        if (QUEUED..=WRITING).contains(&phase) && deadline != 0 && now >= deadline {
            let expired = if phase == WRITING {
                UNKNOWN
            } else if phase == STARTED {
                EXPIRED_STARTED
            } else {
                EXPIRED
            };
            if self
                .token
                .compare_exchange(
                    token,
                    (token & !MASK) | expired,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                self.notify();
            }
        }
        if self.detection() != Detection::Healthy {
            self.notify();
        }
    }
    pub fn detection(&self) -> Detection {
        if self.stop.load(Ordering::Acquire)
            || self.failed.load(Ordering::Acquire) != 0
            || self.delivery_failed.load(Ordering::Acquire)
        {
            return Detection::MonitorFailed;
        }
        match self.token.load(Ordering::Acquire) & MASK {
            EXPIRED => Detection::QueuedExpired,
            EXPIRED_STARTED => Detection::StartedExpired,
            UNKNOWN => Detection::Unknown,
            _ => Detection::Healthy,
        }
    }
}
struct WorkerGuard {
    signals: Arc<Signals>,
}
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        if !self.signals.stop.load(Ordering::Acquire) {
            self.signals.failed.store(2, Ordering::Release);
            self.signals.notify();
        }
    }
}
impl Watchdog {
    pub fn start() -> std::io::Result<Self> {
        reap();
        SLOTS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < SLOT_LIMIT).then_some(count + 1)
            })
            .map_err(|_| std::io::Error::other("监测资源尚未退出，拒绝启动新线程"))?;
        let now = millis();
        let signals = Arc::new(Signals {
            token: AtomicU64::new(0),
            deadline: AtomicU64::new(0),
            primary: AtomicU64::new(now),
            observer: AtomicU64::new(now),
            failed: AtomicU8::new(0),
            stop: AtomicBool::new(false),
            window: AtomicIsize::new(0),
            delivery_failed: AtomicBool::new(false),
            notice_pending: AtomicBool::new(false),
            notice_attempts: AtomicU8::new(0),
            #[cfg(test)]
            fail_delivery: AtomicBool::new(false),
            #[cfg(test)]
            pause_primary: AtomicBool::new(false),
        });
        let mut task = Retired {
            workers: Vec::with_capacity(2),
            _lease: Lease::new(),
            finished: Arc::new(AtomicBool::new(false)),
        };
        for primary in [true, false] {
            let shared = signals.clone();
            let guard = WorkerGuard {
                signals: shared.clone(),
            };
            let builder = thread::Builder::new().name(
                if primary {
                    "cloudleaf-deadline"
                } else {
                    "cloudleaf-monitor-health"
                }
                .into(),
            );
            #[cfg(test)]
            let fail_spawn = SPAWN_FAILURE.with(|fault| fault.get() == Some(task.workers.len()));
            #[cfg(not(test))]
            let fail_spawn = false;
            let spawned = if fail_spawn {
                Err(std::io::Error::other("故障注入：线程创建失败"))
            } else {
                builder.spawn(move || {
                    let _guard = guard;
                    while !shared.stop.load(Ordering::Acquire) {
                        #[cfg(test)]
                        if primary && shared.pause_primary.load(Ordering::Acquire) {
                            thread::park_timeout(INTERVAL);
                            continue;
                        }
                        shared.tick(primary);
                        thread::park_timeout(INTERVAL);
                    }
                })
            };
            match spawned {
                Ok(worker) => task.workers.push(worker),
                Err(error) => {
                    signals.failed.store(3, Ordering::Release);
                    signals.stop.store(true, Ordering::Release);
                    for worker in &task.workers {
                        worker.thread().unpark();
                    }
                    retire(task);
                    return Err(error);
                }
            }
        }
        Ok(Self {
            signals,
            retired: Some(task),
        })
    }
    pub fn set_window(&self, window: HWND) {
        self.signals
            .window
            .store(window.0 as isize, Ordering::Release);
    }
    pub fn healthy(&self) -> bool {
        self.signals.detection() == Detection::Healthy
    }
    pub fn completion(&self) -> Arc<AtomicBool> {
        self.retired.as_ref().unwrap().finished.clone()
    }
    pub fn detection(&self) -> Detection {
        self.signals.detection()
    }
    #[cfg(test)]
    pub fn inject_delivery_failure(&self, failed: bool) {
        self.signals.fail_delivery.store(failed, Ordering::Release);
    }
    #[cfg(test)]
    pub fn inject_primary_stall(&self) {
        self.signals.pause_primary.store(true, Ordering::Release);
    }
    #[cfg(test)]
    pub fn notification_attempts(&self) -> u8 {
        self.signals.notice_attempts.load(Ordering::Acquire)
    }
    pub fn begin(&self, id: u64, timeout: Duration) -> bool {
        if !self.healthy() || id == 0 || id > (u64::MAX >> 4) {
            return false;
        }
        if self.signals.token.load(Ordering::Acquire) & MASK != 0 {
            return false;
        }
        self.signals.deadline.store(
            millis().saturating_add(timeout.as_millis() as u64),
            Ordering::Release,
        );
        self.signals.notice_pending.store(false, Ordering::Release);
        self.signals
            .token
            .store((id << 4) | QUEUED, Ordering::Release);
        true
    }
    pub fn start_operation(&self, id: u64) -> bool {
        self.signals
            .token
            .compare_exchange(
                (id << 4) | QUEUED,
                (id << 4) | STARTED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
    pub fn writing(&self, id: u64) -> bool {
        self.signals
            .token
            .compare_exchange(
                (id << 4) | STARTED,
                (id << 4) | WRITING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
    pub fn finish(&self, id: u64) -> bool {
        let token = self.signals.token.load(Ordering::Acquire);
        if token >> 4 != id || ![STARTED, WRITING].contains(&(token & MASK)) {
            return false;
        }
        self.signals
            .token
            .compare_exchange(token, id << 4, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub fn cancel(&self, id: u64) {
        let token = self.signals.token.load(Ordering::Acquire);
        if token >> 4 == id {
            let _ = self.signals.token.compare_exchange(
                token,
                id << 4,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }
    pub fn stop(&self) {
        self.signals.stop.store(true, Ordering::Release);
        self.signals.window.store(0, Ordering::Release);
        if let Some(task) = &self.retired {
            for worker in &task.workers {
                worker.thread().unpark();
            }
        }
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        self.stop();
        if let Some(task) = self.retired.take() {
            retire(task);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait_until(mut test: impl FnMut() -> bool) {
        let until = Instant::now() + Duration::from_secs(3);
        while !test() {
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn independent_detection_needs_no_sta_message_or_input() {
        let watchdog = Watchdog::start().unwrap();
        assert!(watchdog.begin(1, Duration::from_millis(25)));
        wait_until(|| watchdog.detection() != Detection::Healthy);
        assert_eq!(watchdog.detection(), Detection::QueuedExpired);
        assert!(!watchdog.start_operation(1));
    }
    #[test]
    fn writing_timeout_is_unknown_and_cannot_be_confirmed_or_retried() {
        let watchdog = Watchdog::start().unwrap();
        assert!(watchdog.begin(1, Duration::from_millis(25)));
        assert!(watchdog.start_operation(1));
        assert!(watchdog.writing(1));
        wait_until(|| watchdog.detection() == Detection::Unknown);
        assert!(!watchdog.finish(1));
        assert!(!watchdog.begin(2, Duration::from_millis(25)));
    }
    #[test]
    fn observer_detects_failed_primary_monitor_and_disables_admission() {
        let watchdog = Watchdog::start().unwrap();
        watchdog
            .signals
            .pause_primary
            .store(true, Ordering::Release);
        wait_until(|| watchdog.detection() == Detection::MonitorFailed);
        assert!(!watchdog.begin(1, Duration::from_millis(25)));
    }
    #[test]
    fn worker_shutdown_releases_module_leases_without_blocking_sta() {
        let watchdog = Watchdog::start().unwrap();
        let workers: Vec<_> = watchdog
            .retired
            .as_ref()
            .unwrap()
            .workers
            .iter()
            .map(|worker| worker.thread().id())
            .collect();
        assert_eq!(workers.len(), 2);
        watchdog.stop();
        wait_until(|| {
            watchdog
                .retired
                .as_ref()
                .unwrap()
                .workers
                .iter()
                .all(JoinHandle::is_finished)
        });
        assert!(!watchdog.healthy());
        drop(watchdog);
        reap();
    }
    #[test]
    fn partial_worker_start_failure_is_retired_and_does_not_poison_future_monitors() {
        for index in [0, 1] {
            SPAWN_FAILURE.with(|fault| fault.set(Some(index)));
            let result = Watchdog::start();
            SPAWN_FAILURE.with(|fault| fault.set(None));
            assert!(result.is_err());
            let monitor = Watchdog::start().unwrap();
            assert!(monitor.healthy());
            let completion = monitor.completion();
            monitor.stop();
            drop(monitor);
            wait_until(|| {
                reap();
                completion.load(Ordering::Acquire)
            });
        }
    }

    #[test]
    fn completion_is_owner_proof_after_os_thread_handles_signal() {
        let monitor = Watchdog::start().unwrap();
        let completed = monitor.completion();
        monitor.stop();
        wait_until(|| {
            monitor
                .retired
                .as_ref()
                .unwrap()
                .workers
                .iter()
                .all(|worker| {
                    // 安全：真实线程句柄零等待是 OS 退出证据；没有等待 STA 或 COM。
                    unsafe {
                        WaitForSingleObject(HANDLE(worker.as_raw_handle()), 0) == WAIT_OBJECT_0
                    }
                })
        });
        assert!(!completed.load(Ordering::Acquire));
        drop(monitor);
        wait_until(|| {
            reap();
            completed.load(Ordering::Acquire)
        });
    }
}
