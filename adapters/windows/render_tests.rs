//! 原创原生事务故障宿主：实际调用生产 COM 回调、输入核心、render 和写入确认。
//! 词典与初始组合直接布置；断言阶段通过生产 Edit/KeyEdit/Cleanup 的 COM 接口执行。
//! 未参与测试的接口明确返回 E_NOTIMPL，不能代替真实 Windows 文本宿主验收。
#![allow(unused_variables, non_snake_case)]
use super::*;
use windows::Win32::Foundation::{E_NOTIMPL, S_OK};

#[derive(Default)]
struct Data {
    // UTF-16 文本只属于测试文档；从不读取系统应用或用户输入。
    text: RefCell<Vec<u16>>,
    writes: Cell<usize>,
    endings: Cell<usize>,
    mode: Cell<u8>,
    pending: RefCell<VecDeque<ITfEditSession>>,
    host: RefCell<std::rc::Weak<Host>>,
    calls: RefCell<Vec<&'static str>>,
    write_owner: Cell<u8>,
    selection: RefCell<Option<ITfRange>>,
    blocked_detection: Cell<Option<Detection>>,
    blocked_quarantine: Cell<bool>,
    exit_probe: RefCell<Option<Arc<ExitProbe>>>,
}
#[derive(Default)]
struct ExitProbe {
    synchronous: bool,
    writes: std::sync::atomic::AtomicUsize,
    first_unit: AtomicU64,
    instance: AtomicU64,
    epoch: AtomicU64,
    returned: std::sync::atomic::AtomicBool,
}
// 独立故障编号，避免覆盖既有mode=10的EndComposition重入回归。
const EXIT_AFTER_WRITE: u8 = 11;

unsafe extern "system" fn abrupt_owner(parameter: *mut core::ffi::c_void) -> u32 {
    // 安全：隔离测试父线程持有Arc直到真实OS句柄signaled；绝不转移COM/Rc。
    let probe = unsafe { &*(parameter.cast::<Arc<ExitProbe>>()) };
    let _apartment = ReviewApartment::new();
    let (host, data, _context) = native_fixture();
    host.start_platform().unwrap();
    *data.exit_probe.borrow_mut() = Some(probe.clone());
    probe.instance.store(host.instance, Ordering::Release);
    probe.epoch.store(host.epoch.get(), Ordering::Release);
    if probe.synchronous {
        host.scope_proof.set(None);
        data.mode.set(EXIT_AFTER_WRITE);
        native(&host, b'N' as u16);
    } else {
        host.scope_proof.set(Some(Instant::now()));
        native(&host, b'N' as u16);
        native(&host, b'I' as u16);
        native(
            &host,
            windows::Win32::UI::Input::KeyboardAndMouse::VK_SPACE.0,
        );
        data.mode.set(EXIT_AFTER_WRITE);
        drain(&data);
    }
    probe.returned.store(true, Ordering::Release);
    1
}

#[test]
fn round4_abrupt_settext_exit_preserves_flight_without_drop_or_retry() {
    use super::super::{budget, recovery, service::DllCanUnloadNow};
    use windows::Win32::{
        Foundation::{CloseHandle, S_FALSE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{
            CreateThread, GetExitCodeThread, THREAD_CREATION_FLAGS, WaitForSingleObject,
        },
    };
    if recovery::isolated(
        "native::session::render_tests::round4_abrupt_settext_exit_preserves_flight_without_drop_or_retry",
    ) {
        return;
    }
    for (index, synchronous) in [false, true].into_iter().enumerate() {
        let probe = Arc::new(ExitProbe {
            synchronous,
            ..ExitProbe::default()
        });
        // 仅自有Win32故障线程；ExitThread不跑Rust析构，不用std JoinHandle完成包。
        let handle = unsafe {
            CreateThread(
                None,
                0,
                Some(abrupt_owner),
                Some((&probe as *const Arc<ExitProbe>).cast()),
                THREAD_CREATION_FLAGS(0),
                None,
            )
            .unwrap()
        };
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let result = unsafe { WaitForSingleObject(handle, 0) };
            if result == WAIT_OBJECT_0 {
                break;
            }
            assert_eq!(result, WAIT_TIMEOUT);
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut exit = 0;
        unsafe {
            GetExitCodeThread(handle, &mut exit).unwrap();
            CloseHandle(handle).unwrap();
        }
        assert_eq!(exit, 77);
        assert!(!probe.returned.load(Ordering::Acquire));
        assert_eq!(probe.writes.load(Ordering::Acquire), 1);
        assert_eq!(probe.first_unit.load(Ordering::Acquire), u64::from(b'n'));
        assert_eq!(DllCanUnloadNow(), S_FALSE);
        assert_eq!(budget::usage().batches, index + 1);
        let instance = probe.instance.load(Ordering::Acquire);
        let epoch = probe.epoch.load(Ordering::Acquire);
        let _apartment = ReviewApartment::new();
        let reader = Host::new().unwrap();
        let view = recovery::RecoveryWindow::new(Rc::downgrade(&reader), 0xbeef).unwrap();
        let shown = view.explicit_all_for_review();
        let count = if synchronous { 1 } else { 3 };
        for sequence in 1..=count {
            assert_eq!(
                shown
                    .matches(&format!(
                        "instance={instance} epoch={epoch} sequence={sequence} "
                    ))
                    .count(),
                1
            );
        }
        assert!(shown.contains("sequence=1 state=Unknown"));
        assert!(shown.contains("计划目标（仅供核对；Unknown不得重试）：n"));
        assert!(shown.contains("原始键=Char('n') VK=78 LPARAM=1"));
        if !synchronous {
            assert!(shown.contains("sequence=2 state=NotApplied"));
            assert!(shown.contains("sequence=3 state=NotApplied"));
        }
        view.explicit_discard_all_for_review();
        assert!(view.explicit_all_for_review().is_empty());
        drop(view);
        drop(reader);
        assert_eq!(DllCanUnloadNow(), S_FALSE);
        // 未析构Host仍持有同一epoch凭证；目录清空不等于其预算可以提前退还。
        assert_eq!(budget::usage().batches, index + 1);
        // 故意违约的未析构Host/COM仍真实保活并计费，不能跨线程释放冒充S_OK。
        assert_eq!(budget::usage().hosts, index + 1);
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(probe.writes.load(Ordering::Acquire), 1);
    }
    println!(
        "{{\"experiment\":\"r4-owned-os-thread-write-then-ExitThread\",\"cases\":2,\"writes_per_case\":1,\"materials_reachable\":true,\"material_directory_empty_after_explicit_discard\":true,\"unwound\":false,\"remaining_hosts\":2,\"remaining_epoch_reservations\":2,\"dll_unload\":\"S_FALSE\",\"registered\":false}}"
    );
}

unsafe extern "system" fn round5_abrupt_owner(parameter: *mut core::ffi::c_void) -> u32 {
    // 仅自有故障线程；父线程在真实OS句柄结束之前保持此Arc槽存活。
    let probe = unsafe { &*(parameter.cast::<Arc<ExitProbe>>()) };
    let _apartment = ReviewApartment::new();
    let (host, data, _) = fixture();
    host.start_platform().unwrap();
    *data.exit_probe.borrow_mut() = Some(probe.clone());
    probe.instance.store(host.instance, Ordering::Release);
    probe.epoch.store(host.epoch.get(), Ordering::Release);
    host.scope_proof.set(Some(Instant::now()));
    assert!(native(&host, b'N' as u16));
    data.mode.set(EXIT_AFTER_WRITE);
    drain(&data);
    probe.returned.store(true, Ordering::Release);
    1
}

#[test]
fn round5_abrupt_owner_workers_reclaim_slots_before_healthy_fifo_session() {
    use super::super::{budget, recovery, service::DllCanUnloadNow};
    use windows::Win32::{
        Foundation::{CloseHandle, S_FALSE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{
            CreateThread, GetExitCodeThread, THREAD_CREATION_FLAGS, WaitForSingleObject,
        },
    };
    if recovery::isolated(
        "native::session::render_tests::round5_abrupt_owner_workers_reclaim_slots_before_healthy_fifo_session",
    ) {
        return;
    }
    let started = Instant::now();
    let mut probes = Vec::new();
    for index in 0..32 {
        let probe = Arc::new(ExitProbe::default());
        let handle = unsafe {
            CreateThread(
                None,
                0,
                Some(round5_abrupt_owner),
                Some((&probe as *const Arc<ExitProbe>).cast()),
                THREAD_CREATION_FLAGS(0),
                None,
            )
            .unwrap()
        };
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let outcome = unsafe { WaitForSingleObject(handle, 0) };
            if outcome == WAIT_OBJECT_0 {
                break;
            }
            assert_eq!(outcome, WAIT_TIMEOUT);
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut exit = 0;
        unsafe {
            GetExitCodeThread(handle, &mut exit).unwrap();
            CloseHandle(handle).unwrap();
        }
        assert_eq!(exit, 77);
        assert!(!probe.returned.load(Ordering::Acquire));
        assert_eq!(probe.writes.load(Ordering::Acquire), 1);
        assert_eq!(DllCanUnloadNow(), S_FALSE);
        assert_eq!(budget::usage().hosts, index + 1);
        assert_eq!(budget::usage().batches, index + 1);
        probes.push(probe);
        // 仅故障宿主工程测试等待已停工线程，不在生产按键路径等待。
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(DllCanUnloadNow(), S_FALSE);
    }
    let occupied = budget::usage();
    println!(
        "{{\"experiment\":\"r5-abrupt-owner-occupied\",\"owners\":32,\"fixed_bytes\":{},\"text_bytes\":{},\"global_fixed_limit\":{}}}",
        occupied.fixed_bytes,
        occupied.text_bytes,
        budget::limits().fixed_bytes
    );
    let _apartment = ReviewApartment::new();
    let (healthy, data, _) = fixture();
    let activation = healthy.start_platform();
    assert!(
        activation.is_ok(),
        "32个故障owner停工后，已证明安全的monitor slot必须可供健康会话使用：{activation:?}"
    );
    let extended = std::env::var("CLOUDLEAF_REVIEW_PINNED_SECONDS")
        .map(|value| {
            let seconds = value.parse::<u64>().expect("异常占用延长档必须为整数秒");
            assert!((180..=600).contains(&seconds));
            Duration::from_secs(seconds)
        })
        .unwrap_or_default();
    let steady_started = Instant::now();
    let steady_budget = budget::usage();
    let mut cycles = 0;
    while cycles < 3 || steady_started.elapsed() < extended {
        healthy.scope_proof.set(Some(Instant::now()));
        for key in [b'N', b'I', b'H', b'A', b'O', b' '] {
            assert!(native(&healthy, u16::from(key)));
        }
        drain(&data);
        assert_eq!(text(&data), "你好");
        assert!(healthy.owned.borrow().is_empty());
        assert!(healthy.epoch_budget.borrow().is_none());
        assert_eq!(budget::usage(), steady_budget);
        data.text.borrow_mut().clear();
        data.calls.borrow_mut().clear();
        cycles += 1;
        assert_eq!(data.writes.get(), cycles * 6);
        if !extended.is_zero() && cycles % 1000 == 0 {
            println!(
                "{{\"experiment\":\"r5-pinned-progress\",\"healthy_cycles\":{cycles},\"remaining_hosts\":33,\"remaining_batches\":32,\"pinned_budget_unchanged\":true,\"elapsed_ns\":{}}}",
                steady_started.elapsed().as_nanos()
            );
        }
    }
    let steady_elapsed = steady_started.elapsed();
    assert_eq!(data.writes.get(), cycles * 6);
    let window = healthy.recovery.borrow().as_ref().unwrap().clone();
    let material = window.explicit_all_for_review();
    for probe in &probes {
        let instance = probe.instance.load(Ordering::Acquire);
        let epoch = probe.epoch.load(Ordering::Acquire);
        assert_eq!(
            material
                .matches(&format!("instance={instance} epoch={epoch} sequence=1 "))
                .count(),
            1
        );
        assert_eq!(probe.writes.load(Ordering::Acquire), 1);
    }
    assert_eq!(material.matches("state=Unknown").count(), 32);
    window.explicit_discard_all_for_review();
    assert!(window.explicit_all_for_review().is_empty());
    healthy.stop_platform();
    drop(window);
    drop(healthy);
    let until = Instant::now() + Duration::from_secs(3);
    while budget::usage().hosts != 32 {
        assert!(Instant::now() < until);
        let _ = DllCanUnloadNow();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(budget::usage().batches, 32);
    assert_eq!(DllCanUnloadNow(), S_FALSE);
    println!(
        "{{\"experiment\":\"r5-pinned-availability\",\"faulted_owners\":32,\"healthy_cycles\":{cycles},\"healthy_writes\":{},\"unknown_replays\":0,\"remaining_hosts\":32,\"remaining_batches\":32,\"pinned_budget_unchanged\":true,\"steady_elapsed_ns\":{},\"elapsed_ns\":{},\"registered\":false}}",
        data.writes.get(),
        steady_elapsed.as_nanos(),
        started.elapsed().as_nanos()
    );
    if !extended.is_zero() {
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[test]
fn round5_external_com_references_pin_old_host_without_polluting_new_context() {
    use super::super::{budget, recovery, service::DllCanUnloadNow};
    use windows::Win32::Foundation::{S_FALSE, S_OK};
    if recovery::isolated(
        "native::session::render_tests::round5_external_com_references_pin_old_host_without_polluting_new_context",
    ) {
        return;
    }
    let _apartment = ReviewApartment::new();
    let (old, old_data, _) = fixture();
    old.start_platform().unwrap();
    old.scope_proof.set(Some(Instant::now()));
    assert!(native(&old, b'N' as u16));
    let retained = old_data.pending.borrow_mut().pop_front().unwrap();
    let duplicate = retained.clone();
    let old_instance = old.instance;
    let old_epoch = old.epoch.get();
    old.invalidate().unwrap();
    old.stop_platform();
    let old_weak = Rc::downgrade(&old);
    drop(old);
    assert!(old_weak.upgrade().is_some());
    assert_eq!(DllCanUnloadNow(), S_FALSE);
    let (new, data, _) = fixture();
    new.start_platform().unwrap();
    new.scope_proof.set(Some(Instant::now()));
    assert_ne!(old_instance, new.instance);
    for key in [b'N', b'I', b'H', b'A', b'O', b' '] {
        assert!(native(&new, u16::from(key)));
    }
    drain(&data);
    assert_eq!(text(&data), "你好");
    let frame = new.engine.borrow().frame();
    for callback in [&retained, &duplicate, &retained] {
        unsafe {
            callback.DoEditSession(7).unwrap();
        }
    }
    assert_eq!(data.writes.get(), 6);
    assert_eq!(old_data.writes.get(), 0);
    assert_eq!(text(&old_data), "");
    assert_eq!(new.engine.borrow().frame(), frame);
    let window = new.recovery.borrow().as_ref().unwrap().clone();
    let material = window.explicit_all_for_review();
    assert!(material.contains(&format!(
        "instance={old_instance} epoch={old_epoch} sequence=1 "
    )));
    assert!(material.contains("state=NotApplied"));
    drop(retained);
    assert!(old_weak.upgrade().is_some());
    assert_eq!(budget::usage().hosts, 2);
    drop(duplicate);
    assert!(old_weak.upgrade().is_none());
    // 材料和reader可能仍共享原凭证，最终确认在显式放弃和正常退出之后。
    window.explicit_discard_all_for_review();
    new.stop_platform();
    drop(window);
    drop(new);
    let until = Instant::now() + Duration::from_secs(3);
    while DllCanUnloadNow() != S_OK {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(budget::usage(), budget::Usage::default());
    println!(
        "{{\"experiment\":\"r5-external-com-isolation\",\"retained_callbacks\":2,\"late_calls\":3,\"old_writes\":0,\"new_writes\":6,\"final_four_counters_zero\":true,\"registered\":false}}"
    );
}

#[implement(ITfRange, Agile = false)]
struct Range {
    data: Rc<Data>,
    start: Cell<usize>,
    end: Cell<usize>,
}
fn range(data: &Rc<Data>) -> ITfRange {
    Range {
        data: data.clone(),
        start: Cell::new(0),
        end: Cell::new(data.text.borrow().len()),
    }
    .into()
}
#[implement(ITfContext, ITfContextComposition, Agile = false)]
struct Context {
    data: Rc<Data>,
}
#[implement(ITfComposition, Agile = false)]
struct Composition {
    data: Rc<Data>,
    range: ITfRange,
}
#[implement(ITfReadOnlyProperty, Agile = false)]
struct Property;
#[implement(ITfContextView, Agile = false)]
struct Layout;
fn fixture() -> (Shared, Rc<Data>, ITfContext) {
    let host = Host::new().unwrap();
    let data = Rc::new(Data::default());
    *data.host.borrow_mut() = Rc::downgrade(&host);
    let context: ITfContext = Context { data: data.clone() }.into();
    *host.context.borrow_mut() = Some(context.clone());
    host.engine.borrow_mut().handle(InputEvent::Activate);
    host.projection.borrow_mut().handle(InputEvent::Activate);
    (host, data, context)
}

fn native_fixture() -> (Shared, Rc<Data>, ITfContext) {
    let (host, data, context) = fixture();
    host.projection.borrow_mut().handle(InputEvent::Activate);
    *host.watchdog.borrow_mut() = Some(Watchdog::start().unwrap());
    host.scope_proof.set(Some(Instant::now()));
    (host, data, context)
}
struct ReviewApartment;
impl ReviewApartment {
    fn new() -> Self {
        // 安全：独立测试owner初始化自己的COM STA；先析构该owner对象，最后对称撤销。
        unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            )
            .ok()
            .unwrap();
        }
        Self
    }
}
impl Drop for ReviewApartment {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }
    }
}

#[test]
fn round4_physical_sta_exit_releases_receipt_but_preserves_unknown_until_new_sta_discards() {
    use super::super::{budget, recovery, service::DllCanUnloadNow};
    use windows::Win32::Foundation::{S_FALSE, S_OK};
    if recovery::isolated(
        "native::session::render_tests::round4_physical_sta_exit_releases_receipt_but_preserves_unknown_until_new_sta_discards",
    ) {
        return;
    }
    let (instance, epoch, material) = std::thread::spawn(|| {
        let _apartment = ReviewApartment::new();
        let (host, data, _context) = native_fixture();
        host.start_platform().unwrap();
        host.scope_proof.set(None);
        data.mode.set(2);
        assert!(native(&host, b'N' as u16));
        assert_eq!(data.writes.get(), 1);
        assert_eq!(text(&data), "n");
        let window = host.recovery.borrow().as_ref().unwrap().clone();
        let identity = (host.instance, host.epoch.get());
        host.stop_platform();
        let evidence = window.evidence();
        drop(window);
        drop(host);
        assert_eq!(data.writes.get(), 1);
        (identity.0, identity.1, evidence)
    })
    .join()
    .unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    while !recovery::receipt_snapshot_for_review().is_empty() {
        assert!(Instant::now() < until);
        assert_eq!(DllCanUnloadNow(), S_FALSE);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(budget::usage().hosts, 0);
    assert_eq!(budget::usage().batches, 1);
    assert_eq!(DllCanUnloadNow(), S_FALSE);
    let _apartment = ReviewApartment::new();
    let host = Host::new().unwrap();
    let view = recovery::RecoveryWindow::new(Rc::downgrade(&host), 0xbeef).unwrap();
    let shown = view.explicit_all_for_review();
    assert_eq!(shown, material);
    assert!(shown.contains(&format!("instance={instance} epoch={epoch} sequence=1")));
    assert!(shown.contains("state=Unknown"));
    view.explicit_discard_all_for_review();
    drop(view);
    drop(host);
    assert_eq!(DllCanUnloadNow(), S_OK);
    assert_eq!(budget::usage(), budget::Usage::default());
}

#[test]
fn round4_multista_unknown_exit_and_class_failure_recover_for_twenty_rounds() {
    use super::super::{budget, recovery, service::DllCanUnloadNow};
    use windows::Win32::Foundation::{S_FALSE, S_OK};
    if recovery::isolated(
        "native::session::render_tests::round4_multista_unknown_exit_and_class_failure_recover_for_twenty_rounds",
    ) {
        return;
    }
    let started = Instant::now();
    let extended = std::env::var("CLOUDLEAF_REVIEW_STRESS_SECONDS")
        .map(|seconds| {
            let seconds = seconds.parse::<u64>().expect("延长档必须是整数秒");
            assert!((180..=600).contains(&seconds), "延长档仅允许180至600秒");
            Duration::from_secs(seconds)
        })
        .unwrap_or_default();
    if !extended.is_zero() {
        println!(
            "{{\"experiment\":\"r4-owner-stress-start\",\"pid\":{},\"requested_seconds\":{}}}",
            std::process::id(),
            extended.as_secs()
        );
    }
    let mut tested = 0;
    let mut highest_fixed = 0;
    let mut round = 0;
    while round < 20 || started.elapsed() < extended {
        let owners: Vec<_> = (0..4)
            .map(|_| {
                std::thread::spawn(move || {
                    let _apartment = ReviewApartment::new();
                    let (host, data, _context) = native_fixture();
                    host.start_platform().unwrap();
                    host.scope_proof.set(Some(Instant::now()));
                    data.mode.set(2);
                    assert!(native(&host, b'N' as u16));
                    assert!(native(&host, b'I' as u16));
                    assert!(native(
                        &host,
                        windows::Win32::UI::Input::KeyboardAndMouse::VK_SPACE.0
                    ));
                    drain(&data);
                    assert_eq!(data.writes.get(), 1);
                    assert!(host.quarantined.get());
                    let window = host.recovery.borrow().as_ref().unwrap().clone();
                    window.inject_cleanup_failure(round % 2 == 0);
                    let identity = (
                        host.instance,
                        host.epoch.get(),
                        window.receipt_id_for_review(),
                    );
                    host.stop_platform();
                    let evidence = window.evidence();
                    assert!(evidence.contains("state=Unknown"));
                    drop(window);
                    drop(host);
                    assert_eq!(data.writes.get(), 1);
                    (identity, evidence)
                })
            })
            .collect();
        let records: Vec<_> = owners
            .into_iter()
            .map(|owner| owner.join().unwrap())
            .collect();
        tested += records.len();
        highest_fixed = highest_fixed.max(budget::usage().fixed_bytes);
        assert_eq!(DllCanUnloadNow(), S_FALSE);
        if round % 2 == 0 {
            // 零等待回收须先取得worker真实退出证明，再观察已注入的类清理失败。
            let until = Instant::now() + Duration::from_secs(3);
            loop {
                let receipts = recovery::receipt_snapshot_for_review();
                if receipts.len() == 4
                    && receipts
                        .iter()
                        .all(|receipt| receipt.last_error != 0 && receipt.attempts > 0)
                {
                    break;
                }
                assert!(
                    Instant::now() < until,
                    "未在期限内观察到四个清理失败：{receipts:?}"
                );
                assert_eq!(DllCanUnloadNow(), S_FALSE);
                std::thread::sleep(Duration::from_millis(2));
            }
            let receipts = recovery::receipt_snapshot_for_review();
            assert_eq!(receipts.len(), 4);
            for ((instance, _, generation), _) in &records {
                let receipt = receipts
                    .iter()
                    .find(|receipt| receipt.generation == *generation)
                    .unwrap();
                assert_eq!(receipt.instance, *instance);
                assert!(receipt.owner_exited);
                assert!(receipt.last_error != 0 && receipt.attempts > 0);
                recovery::clear_cleanup_failure_for_review(*generation);
            }
        }
        let until = Instant::now() + Duration::from_secs(3);
        while !recovery::receipt_snapshot_for_review().is_empty() {
            assert!(Instant::now() < until);
            assert_eq!(DllCanUnloadNow(), S_FALSE);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(budget::usage().hosts, 0);
        assert_eq!(budget::usage().batches, 4);
        {
            let _apartment = ReviewApartment::new();
            let host = Host::new().unwrap();
            let view = recovery::RecoveryWindow::new(Rc::downgrade(&host), 0xbeef).unwrap();
            let shown = view.explicit_all_for_review();
            for ((instance, epoch, _), material) in records {
                assert!(shown.contains(&material));
                for sequence in 1..=3 {
                    assert_eq!(
                        shown
                            .matches(&format!(
                                "instance={instance} epoch={epoch} sequence={sequence} "
                            ))
                            .count(),
                        1
                    );
                }
            }
            view.explicit_discard_all_for_review();
        }
        assert_eq!(DllCanUnloadNow(), S_OK);
        assert_eq!(budget::usage(), budget::Usage::default());
        round += 1;
        if !extended.is_zero() && round % 20 == 0 {
            println!(
                "{{\"experiment\":\"r4-owner-stress-progress\",\"rounds\":{round},\"owners\":{tested},\"all_four_counters_zero\":true,\"dll_unload\":\"S_OK\",\"elapsed_ns\":{}}}",
                started.elapsed().as_nanos()
            );
        }
    }
    println!(
        "{{\"experiment\":\"r4-multista-real-com-unknown-exit\",\"rounds\":{round},\"owners\":{tested},\"faulted_rounds\":{},\"peak_charged_fixed\":{highest_fixed},\"elapsed_ns\":{},\"all_rounds_returned_to_zero\":true,\"registered\":false}}",
        (round + 1) / 2,
        started.elapsed().as_nanos()
    );
    if !extended.is_zero() {
        // 工程父进程仍可采样最终清理后的句柄/线程；不在生产输入路径等待。
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn native(host: &Shared, virtual_key: u16) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardState, SetKeyboardState};
    struct RestoreKeyboard([u8; 256]);
    impl Drop for RestoreKeyboard {
        fn drop(&mut self) {
            // 安全：只还原当前测试线程的输入状态表，不影响其他线程或全局键盘指示灯。
            unsafe { SetKeyboardState(&self.0).unwrap() };
        }
    }
    let mut previous = [0u8; 256];
    // 安全：缓冲区完整有效；测试固定无修饰状态，不依赖操作人员当前 Caps Lock。
    unsafe { GetKeyboardState(&mut previous).unwrap() };
    let _restore = RestoreKeyboard(previous);
    unsafe { SetKeyboardState(&[0u8; 256]).unwrap() };
    host.native_key(
        windows::Win32::Foundation::WPARAM(virtual_key as usize),
        windows::Win32::Foundation::LPARAM(1),
    )
    .unwrap()
}

#[test]
fn round3_cross_instance_budget_rejects_async_sync_and_candidate_before_ownership() {
    use super::super::{budget, recovery, service::DllCanUnloadNow};
    use windows::Win32::{
        Foundation::{LPARAM, S_OK, WPARAM},
        UI::WindowsAndMessaging::*,
    };
    if recovery::isolated(
        "native::session::render_tests::round3_cross_instance_budget_rejects_async_sync_and_candidate_before_ownership",
    ) {
        return;
    }
    let started = Instant::now();
    let mut retained = Vec::new();
    for _ in 0..64 {
        let (host, data, _) = native_fixture();
        host.start_platform().unwrap();
        let before = host.engine.borrow().frame();
        data.mode.set(2);
        if !native(&host, b'N' as u16) {
            assert!(host.owned.borrow().is_empty());
            assert_eq!(data.writes.get(), 0);
            assert_eq!(host.engine.borrow().frame(), before);
            host.stop_platform();
            break;
        }
        drain(&data);
        assert_eq!(data.writes.get(), 1);
        assert!(host.quarantined.get());
        let window = host.recovery.borrow().as_ref().unwrap().clone();
        let original = window.evidence();
        assert!(original.contains("state=Unknown"));
        assert_eq!(original.matches("sequence=1 ").count(), 1);
        host.invalidate().unwrap();
        host.stop_platform();
        let final_material = window.evidence();
        retained.push((window, final_material));
    }
    assert!(!retained.is_empty() && retained.len() < 64);
    let admitted = retained.len();
    assert_eq!(budget::usage().batches, admitted);
    {
        let (host, data, _) = native_fixture();
        host.start_platform().unwrap();
        host.scope_proof.set(None);
        let before = host.engine.borrow().frame();
        assert!(!native(&host, b'N' as u16));
        assert_eq!(data.writes.get(), 0);
        assert_eq!(host.engine.borrow().frame(), before);
        assert!(host.pending_write.borrow().is_none());
    }
    {
        let (host, data, _) = fixture();
        host.start_platform().unwrap();
        seed_composition(&host, &data);
        let before = host.engine.borrow().frame();
        let original = text(&data);
        assert!(
            host.select(before.revision, before.candidates[0].id.clone())
                .is_err()
        );
        assert!(host.cancel().is_err());
        assert_eq!(data.writes.get(), 0);
        assert_eq!(text(&data), original);
        assert_eq!(host.engine.borrow().frame(), before);
    }
    let (text_reservation, fixed_reservation) = recovery_cost();
    println!(
        "{{\"experiment\":\"round3-production-com-global-pressure\",\"hosts_retained\":{admitted},\"epoch_text_reservation\":{text_reservation},\"epoch_fixed_reservation\":{fixed_reservation},\"host_size\":{},\"owned_event_size\":{},\"candidate_size\":{},\"ui_fixed_cost\":{},\"elapsed_ns\":{},\"usage\":\"{:?}\"}}",
        std::mem::size_of::<Host>(),
        std::mem::size_of::<OwnedEvent>(),
        std::mem::size_of::<cloudleaf_types::Candidate>(),
        recovery::fixed_cost(),
        started.elapsed().as_nanos(),
        budget::usage()
    );
    for (window, original) in retained {
        assert_eq!(window.evidence(), original);
        unsafe {
            SendMessageW(window.handle(), WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        drop(window);
    }
    let until = Instant::now() + Duration::from_secs(3);
    while DllCanUnloadNow() != S_OK {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(budget::usage(), budget::Usage::default());
}

#[test]
fn round3_sync_candidate_unknown_retains_reserved_material_and_applied_releases_it() {
    use super::super::{budget, recovery};
    if recovery::isolated(
        "native::session::render_tests::round3_sync_candidate_unknown_retains_reserved_material_and_applied_releases_it",
    ) {
        return;
    }
    let (host, data, _) = native_fixture();
    host.start_platform().unwrap();
    assert!(native(&host, b'N' as u16));
    assert!(native(&host, b'I' as u16));
    drain(&data);
    assert!(host.epoch_budget.borrow().is_none());
    assert_eq!(budget::usage().batches, 0);
    let before = host.engine.borrow().frame();
    data.mode.set(2);
    assert!(
        host.select(before.revision, before.candidates[0].id.clone())
            .is_err()
    );
    assert_eq!(budget::usage().batches, 1);
    let evidence = host.recovery.borrow().as_ref().unwrap().evidence();
    assert!(evidence.contains("同步平台操作=候选 Finalize"));
    assert!(evidence.contains("state=Unknown"));
    assert!(evidence.contains("不是原生按键信封"));
    let writes = data.writes.get();
    host.watchdog_notice().unwrap();
    assert_eq!(data.writes.get(), writes);
    // 此 fixture 不实现焦点 sink 的 ITfSource；明确放弃已生效，重新聚焦仍须报告真实接口失败。
    assert_eq!(
        host.discard_recovery().unwrap_err().code(),
        windows::Win32::Foundation::E_NOINTERFACE
    );
    drain(&data);
    assert_eq!(budget::usage().batches, 0);
}

#[test]
fn round3_expired_scope_sync_native_unknown_preserves_true_envelope_after_stop_and_new_view() {
    use super::super::{budget, recovery};
    if recovery::isolated(
        "native::session::render_tests::round3_expired_scope_sync_native_unknown_preserves_true_envelope_after_stop_and_new_view",
    ) {
        return;
    }
    let (host, data, _) = native_fixture();
    host.start_platform().unwrap();
    host.scope_proof.set(None);
    data.mode.set(2);
    assert!(native(&host, b'N' as u16));
    assert_eq!(text(&data), "n");
    assert_eq!(data.writes.get(), 1);
    let native = host
        .pending_write
        .borrow()
        .as_ref()
        .unwrap()
        .native
        .clone()
        .unwrap();
    assert_eq!(native.wparam, b'N' as usize);
    assert_eq!(native.lparam, 1);
    assert_eq!(native.key, Key::Char('n'));
    assert_eq!(native.sequence, 1);
    assert_eq!(native.repeat, 1);
    let window = host.recovery.borrow().as_ref().unwrap().clone();
    host.stop_platform();
    let original = window.evidence();
    assert!(original.contains("state=Unknown"));
    assert!(original.contains("真实原生信封"));
    assert!(original.contains("原始键=Char('n') VK=78 LPARAM=1"));
    assert!(original.contains("计划目标（可能已写入，不是可重试的未应用文本）：n"));
    assert!(original.contains(&format!(
        "instance={} epoch={} sequence={}",
        native.instance, native.epoch, native.sequence
    )));
    drop(host);
    let recovered = std::thread::spawn(|| {
        let host = Host::new().unwrap();
        let view = recovery::RecoveryWindow::new(Rc::downgrade(&host), 0xbeef).unwrap();
        view.explicit_all_for_review()
    })
    .join()
    .unwrap();
    assert_eq!(recovered, original);
    assert_eq!(data.writes.get(), 1);
    assert_eq!(budget::usage().batches, 1);
    window.discard_epoch(native.epoch).unwrap();
    assert_eq!(budget::usage().batches, 0);
}

#[test]
fn native_owned_fifo_drains_before_second_space_native_command_and_rejects_late_callback() {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_SPACE;
    let (host, data, _) = native_fixture();
    assert!(native(&host, b'N' as u16));
    assert!(native(&host, b'I' as u16));
    assert!(native(&host, VK_SPACE.0));
    assert_eq!(data.writes.get(), 0);
    assert_eq!(host.owned.borrow().len(), 3);
    assert!(!native(&host, VK_SPACE.0));
    assert_eq!(text(&data), "你");
    assert!(host.owned.borrow().is_empty());
    let writes = data.writes.get();
    // 原生第二个 Space 仍属于应用；由测试应用插入，晚到旧回调不能覆盖或插入额外空格。
    data.text.borrow_mut().push(' ' as u16);
    drain(&data);
    assert_eq!(text(&data), "你 ");
    assert_eq!(data.writes.get(), writes);
    assert!(!host.quarantined.get());
}

#[test]
fn native_probe_is_readonly_and_queue_full_does_not_take_new_key() {
    let (host, data, _) = native_fixture();
    let before = host.engine.borrow().frame();
    let projection = host.projection.borrow().frame();
    for _ in 0..100 {
        assert!(host.native_probe(Some(Key::Char('n'))).unwrap());
    }
    assert_eq!(before, host.engine.borrow().frame());
    assert_eq!(projection, host.projection.borrow().frame());
    assert_eq!(host.native_sequence.get(), 1);
    for index in 0..QUEUE_LIMIT {
        // 此用例固定有效scope以测容量；过期scope另有独立回归。
        host.scope_proof.set(Some(Instant::now()));
        assert!(native(&host, b'N' as u16));
        assert_eq!(host.owned.borrow().len(), index + 1);
    }
    let next = host.native_sequence.get();
    host.scope_proof.set(Some(Instant::now()));
    assert!(!native(&host, b'N' as u16));
    assert_eq!(host.native_sequence.get(), next);
    assert_eq!(host.owned.borrow().len(), QUEUE_LIMIT);
    assert!(host.quarantined.get());
    drain(&data);
    assert_eq!(data.writes.get(), 0);
}

#[test]
fn round4_expired_scope_with_full_ownership_rejects_before_barrier_or_write() {
    let (host, data, _) = native_fixture();
    for _ in 0..QUEUE_LIMIT {
        // 布置已知有效scope的满队列；容量条件成立后再明确撤销scope证明。
        host.scope_proof.set(Some(Instant::now()));
        assert!(native(&host, b'N' as u16));
    }
    assert_eq!(host.owned.borrow().len(), QUEUE_LIMIT);
    let next = host.native_sequence.get();
    host.scope_proof.set(None);
    data.calls.borrow_mut().clear();
    assert!(!native(&host, b'N' as u16));
    assert_eq!(host.native_sequence.get(), next);
    assert_eq!(host.owned.borrow().len(), QUEUE_LIMIT);
    assert!(host.quarantined.get());
    assert!(data.calls.borrow().is_empty());
    assert_eq!(data.writes.get(), 0);
    drain(&data);
    assert_eq!(data.writes.get(), 0);
}

#[test]
fn native_barrier_rejection_preserves_owned_events_and_does_not_fake_enter() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_RETURN, VK_SPACE};
    let (host, data, _) = native_fixture();
    assert!(native(&host, b'N' as u16));
    assert!(native(&host, VK_SPACE.0));
    data.mode.set(7);
    assert!(!native(&host, VK_RETURN.0));
    assert!(host.quarantined.get());
    assert_eq!(host.owned.borrow().len(), 2);
    assert!(
        host.owned
            .borrow()
            .snapshot()
            .iter()
            .all(|event| event.outcome == Outcome::NotApplied)
    );
    data.mode.set(0);
    drain(&data);
    assert_eq!(data.writes.get(), 0);
    assert_eq!(text(&data), "");
}

#[test]
fn native_async_partial_write_is_unknown_with_no_automatic_retry() {
    let (host, data, _) = native_fixture();
    assert!(native(&host, b'N' as u16));
    data.mode.set(2);
    drain(&data);
    assert!(host.quarantined.get());
    assert_eq!(
        host.owned.borrow().front().unwrap().outcome,
        Outcome::Unknown
    );
    assert_eq!(data.writes.get(), 1);
    data.mode.set(0);
    drain(&data);
    assert!(!native(&host, b'I' as u16));
    assert_eq!(data.writes.get(), 1);
    assert_eq!(text(&data), "n");
}

#[test]
fn native_external_edit_invalidates_scope_and_old_composition_without_erasing_host_text() {
    let (host, data, context) = native_fixture();
    assert!(native(&host, b'N' as u16));
    drain(&data);
    data.text.borrow_mut().extend("应用".encode_utf16());
    let observer: ITfTextEditSink = TextObserver {
        host: Rc::downgrade(&host),
        epoch: host.epoch.get(),
        _lease: super::super::service::Lease::new(),
    }
    .into();
    // 安全：直接注入外部编辑通知，不读取或操作系统应用；旧range的扩大不允许导致删除。
    unsafe {
        observer.OnEndEdit(&context, 7, None).unwrap();
    }
    drain(&data);
    assert_eq!(text(&data), "n应用");
    assert!(host.scope_proof.get().is_none());
    assert!(host.composition.borrow().is_none());
    assert!(!native(&host, b'I' as u16));
    assert_eq!(text(&data), "n应用");
}

#[test]
fn native_end_edit_uses_client_write_ownership_true_false_and_error_without_reading_text() {
    for owner in [0, 1, 2] {
        let (host, data, context) = native_fixture();
        assert!(native(&host, b'N' as u16));
        drain(&data);
        data.write_owner.set(owner);
        let epoch = host.epoch.get();
        let observer: ITfTextEditSink = TextObserver {
            host: Rc::downgrade(&host),
            epoch,
            _lease: super::super::service::Lease::new(),
        }
        .into();
        let writes = data.writes.get();
        // 安全：实际生产 sink 的 COM 通知只拿只读 cookie，外部通知只能异步结束旧组合。
        unsafe {
            observer.OnEndEdit(&context, 7, None).unwrap();
        }
        assert!(host.scope_proof.get().is_none());
        assert!(data.calls.borrow().contains(&"InWriteSession"));
        assert_eq!(data.writes.get(), writes);
        if owner == 1 {
            assert_eq!(host.epoch.get(), epoch);
            assert!(host.composition.borrow().is_some());
            assert!(data.pending.borrow().is_empty());
        } else {
            assert_ne!(host.epoch.get(), epoch);
            assert!(host.composition.borrow().is_none());
            assert_eq!(data.endings.get(), 0);
            assert_eq!(data.pending.borrow().len(), 1);
            drain(&data);
            assert_eq!(data.endings.get(), 1);
            assert_eq!(text(&data), "n");
        }
    }
}

#[test]
fn native_sync_fallback_async_select_cancel_and_next_commit_keep_projection_semantics() {
    let (host, data, _) = native_fixture();
    host.scope_proof.set(None);
    assert!(native(&host, b'N' as u16));
    assert_eq!(text(&data), "n");
    assert!(native(&host, b'I' as u16));
    drain(&data);
    let frame = host.engine.borrow().frame();
    host.select(frame.revision, frame.candidates[0].id.clone())
        .unwrap();
    assert_eq!(text(&data), "你");
    assert!(native(&host, b'N' as u16));
    host.cancel().unwrap();
    assert!(host.owned.borrow().is_empty());
    assert_eq!(host.engine.borrow().frame().preedit, "");
    assert_eq!(host.projection.borrow().frame().preedit, "");
    assert!(native(&host, b'N' as u16));
    assert!(native(&host, b'I' as u16));
    drain(&data);
    assert!(!host.quarantined.get());
    let frame = host.engine.borrow().frame();
    host.select(frame.revision, frame.candidates[0].id.clone())
        .unwrap();
    assert_eq!(text(&data), "你你");
}

#[test]
fn native_unknown_and_focus_reset_do_not_require_shadow_ids_to_equal_actual_ids() {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_SPACE;
    let (host, data, context) = native_fixture();
    assert!(native(&host, b'N' as u16));
    assert!(native(&host, b'I' as u16));
    assert!(native(&host, VK_SPACE.0));
    // 投影已经计划后续一次提交；实际首个写入发生 Unknown，不能给 actual 补假 ACK。
    data.mode.set(2);
    drain(&data);
    assert!(host.quarantined.get());
    data.mode.set(0);
    host.owned.borrow_mut().clear();
    host.unknown_text.set(true);
    host.invalidate().unwrap();
    drain(&data);
    *host.context.borrow_mut() = Some(context);
    // 模拟用户在恢复后的新目标选择原文末尾；没有自动搬运或回放 Unknown 原文。
    *data.selection.borrow_mut() = Some(
        Range {
            data: data.clone(),
            start: Cell::new(data.text.borrow().len()),
            end: Cell::new(data.text.borrow().len()),
        }
        .into(),
    );
    host.scope_proof.set(Some(Instant::now()));
    assert_ne!(
        host.engine.borrow().frame().revision,
        host.projection.borrow().frame().revision
    );
    assert!(native(&host, b'N' as u16));
    assert!(native(&host, b'I' as u16));
    drain(&data);
    assert!(!host.quarantined.get());
    let frame = host.engine.borrow().frame();
    host.select(frame.revision, frame.candidates[0].id.clone())
        .unwrap();
    assert_eq!(text(&data), "n你");
    assert!(!host.quarantined.get());
}

#[test]
fn native_semantic_comparison_only_excludes_revision_and_commit_id() {
    let (host, data, _) = native_fixture();
    assert!(native(&host, b'N' as u16));
    let planned = host.owned.borrow().front().unwrap().planned.clone();
    let mut actual = planned.clone();
    actual.frame.revision += 19;
    assert!(same_semantics(&actual, &planned));
    actual.frame.cursor += 1;
    assert!(!same_semantics(&actual, &planned));
    let mut actual = planned.clone();
    actual.error = Some(cloudleaf_types::CoreError::new(
        cloudleaf_types::ErrorCode::InvalidInput,
        "模拟 ID 耗尽",
    ));
    assert!(!same_semantics(&actual, &planned));
    drain(&data);
}

#[test]
fn native_empty_composition_delete_has_the_same_unknown_boundary_as_nonempty_writes() {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
    for mode in [0u8, 2, 4, 9] {
        let (host, data, _) = native_fixture();
        seed_composition(&host, &data);
        assert!(native(&host, VK_ESCAPE.0));
        data.mode.set(mode);
        drain(&data);
        assert_eq!(text(&data), "");
        assert_eq!(data.writes.get(), 1);
        if mode == 0 {
            assert!(host.owned.borrow().is_empty());
            assert!(!host.quarantined.get());
            assert!(!host.unknown_text.get());
        } else {
            assert!(host.quarantined.get());
            assert!(host.unknown_text.get());
            assert_eq!(
                host.owned.borrow().front().unwrap().outcome,
                Outcome::Unknown
            );
            if mode == 9 {
                assert_eq!(
                    host.watchdog.borrow().as_ref().unwrap().detection(),
                    Detection::Unknown
                );
                assert_eq!(data.blocked_detection.get(), Some(Detection::Unknown));
                assert!(!data.blocked_quarantine.get());
                assert_eq!(data.endings.get(), 0);
            }
            let writes = data.writes.get();
            data.mode.set(0);
            data.text.borrow_mut().extend("应用新文本".encode_utf16());
            host.invalidate().unwrap();
            drain(&data);
            assert_eq!(text(&data), "应用新文本");
            assert_eq!(data.writes.get(), writes);
        }
    }
}

fn platform_fixture() -> (Shared, Rc<Data>, ITfContext) {
    let (host, data, context) = fixture();
    host.start_platform().unwrap();
    host.scope_proof.set(Some(Instant::now()));
    (host, data, context)
}
fn pump_notifications() {
    use windows::Win32::UI::WindowsAndMessaging::*;
    let mut message = MSG::default();
    // 安全：仅分发当前测试 STA 的消息，不向任何应用生成键盘或文本消息。
    unsafe {
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
fn wait_native(mut condition: impl FnMut() -> bool, pump: bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "独立检测或 STA 通知未在诊断期限内到达"
        );
        if pump {
            pump_notifications();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn close_platform_fixture(host: Shared, data: &Data) {
    use windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::*,
    };
    let window = host.recovery.borrow().as_ref().unwrap().handle();
    host.stop_platform();
    drain(data);
    drop(host);
    // 安全：只关闭测试创建的恢复窗口；此时已无 Host，用户放弃材料不会触及宿主文档。
    unsafe { SendMessageW(window, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0))) };
    wait_native(
        || {
            super::super::watchdog::reap();
            super::super::recovery::reap();
            unsafe { !IsWindow(Some(window)).as_bool() }
        },
        true,
    );
}

#[test]
fn native_watchdog_without_more_input_notifies_sta_and_seals_the_late_callback() {
    let (host, data, _) = platform_fixture();
    assert!(native(&host, b'N' as u16));
    wait_native(
        || host.watchdog.borrow().as_ref().unwrap().detection() == Detection::QueuedExpired,
        false,
    );
    assert!(!host.quarantined.get());
    assert_eq!(data.writes.get(), 0);
    wait_native(|| host.quarantined.get(), true);
    assert!(host.recovery.borrow().as_ref().unwrap().has_material());
    assert_eq!(
        host.owned.borrow().front().unwrap().outcome,
        Outcome::NotApplied
    );
    drain(&data);
    assert_eq!(data.writes.get(), 0);
    close_platform_fixture(host, &data);
}

#[test]
fn native_watchdog_failed_notification_is_bounded_and_next_sta_entry_recovers_ownership() {
    let (host, data, _) = platform_fixture();
    host.watchdog
        .borrow()
        .as_ref()
        .unwrap()
        .inject_delivery_failure(true);
    assert!(native(&host, b'N' as u16));
    wait_native(
        || {
            host.watchdog
                .borrow()
                .as_ref()
                .unwrap()
                .notification_attempts()
                == 4
        },
        false,
    );
    assert_eq!(
        host.watchdog.borrow().as_ref().unwrap().detection(),
        Detection::MonitorFailed
    );
    assert!(!host.quarantined.get());
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(
        host.watchdog
            .borrow()
            .as_ref()
            .unwrap()
            .notification_attempts(),
        4
    );
    assert!(!native(&host, b'I' as u16));
    assert!(host.quarantined.get());
    assert!(host.recovery.borrow().as_ref().unwrap().has_material());
    drain(&data);
    assert_eq!(data.writes.get(), 0);
    close_platform_fixture(host, &data);
}

#[test]
fn native_watchdog_peer_failure_stops_admission_and_notifies_the_real_sta_window() {
    let (host, data, _) = platform_fixture();
    assert!(native(&host, b'N' as u16));
    host.watchdog
        .borrow()
        .as_ref()
        .unwrap()
        .inject_primary_stall();
    wait_native(
        || host.watchdog.borrow().as_ref().unwrap().detection() == Detection::MonitorFailed,
        false,
    );
    wait_native(|| host.quarantined.get(), true);
    assert!(host.recovery.borrow().as_ref().unwrap().has_material());
    assert!(!native(&host, b'I' as u16));
    drain(&data);
    assert_eq!(data.writes.get(), 0);
    close_platform_fixture(host, &data);
}

#[test]
fn native_empty_delete_reentrant_focus_cannot_end_or_erase_the_new_context_again() {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
    let (host, data, _) = native_fixture();
    seed_composition(&host, &data);
    assert!(native(&host, VK_ESCAPE.0));
    data.mode.set(3);
    drain(&data);
    assert_eq!(data.writes.get(), 1);
    assert_eq!(data.endings.get(), 1);
    assert_eq!(
        host.owned.borrow().front().unwrap().outcome,
        Outcome::Unknown
    );
    data.text.borrow_mut().extend("应用新文本".encode_utf16());
    drain(&data);
    assert_eq!(text(&data), "应用新文本");
    assert_eq!(data.writes.get(), 1);
    assert_eq!(data.endings.get(), 1);
}

#[test]
fn native_empty_delete_end_reentrant_focus_preserves_the_new_context_composition() {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
    let (host, data, old_context) = native_fixture();
    seed_composition(&host, &data);
    assert!(native(&host, VK_ESCAPE.0));
    data.mode.set(10);
    drain(&data);
    assert_eq!(data.writes.get(), 1);
    assert_eq!(data.endings.get(), 1);
    assert_ne!(host.context.borrow().as_ref(), Some(&old_context));
    assert!(host.composition.borrow().is_some());
    assert_eq!(text(&data), "新上下文文本");
    assert_eq!(
        host.owned.borrow().front().unwrap().outcome,
        Outcome::Unknown
    );
    drain(&data);
    assert_eq!(data.writes.get(), 1);
    assert!(host.composition.borrow().is_some());
}

fn seed_composition(host: &Shared, data: &Rc<Data>) {
    // 直接布置初始组合用于隔离写入阶段；不把这一步算作原生按键成功证据。
    for c in ['n', 'i'] {
        host.engine
            .borrow_mut()
            .handle(InputEvent::Key(Key::Char(c)));
        host.projection
            .borrow_mut()
            .handle(InputEvent::Key(Key::Char(c)));
    }
    *data.text.borrow_mut() = "ni".encode_utf16().collect();
    *host.composition.borrow_mut() = Some(
        Composition {
            data: data.clone(),
            range: range(data),
        }
        .into(),
    );
}
fn execute(host: &Shared, context: &ITfContext) -> Result<()> {
    // 使用生产 Edit 的 COM vtable 进入生产 run/render，并验证其中的实际 ACK 路径。
    *host.pending_layout.borrow_mut() = Some((0, Instant::now()));
    let edit: ITfEditSession = Edit {
        host: host.clone(),
        epoch: host.epoch.get(),
        context: context.clone(),
        operation: 0,
    }
    .into();
    unsafe { edit.DoEditSession(7) }
}
fn text(data: &Data) -> String {
    String::from_utf16(&data.text.borrow()).unwrap()
}
fn drain(data: &Data) {
    loop {
        let next = data.pending.borrow_mut().pop_front();
        let Some(edit) = next else { break };
        unsafe {
            let _ = edit.DoEditSession(7);
        }
    }
}

impl ITfRange_Impl for Range_Impl {
    fn GetText(
        &self,
        ec: u32,
        dwflags: u32,
        pchtext: windows_core::PWSTR,
        cchmax: u32,
        pcch: *mut u32,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn SetText(
        &self,
        ec: u32,
        dwflags: u32,
        pchtext: &windows_core::PCWSTR,
        cch: i32,
    ) -> windows_core::Result<()> {
        self.data.calls.borrow_mut().push("SetText");
        if self.data.mode.get() == 1 {
            return Err(Error::from_hresult(E_FAIL));
        }
        // 安全：生产绑定提供本次调用有效的 UTF-16 切片；空写入无需解引用指针。
        let value = if cch == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(pchtext.0, cch as usize) }.to_vec()
        };
        let start = self.start.get().min(self.data.text.borrow().len());
        let end = self.end.get().min(self.data.text.borrow().len());
        self.data
            .text
            .borrow_mut()
            .splice(start..end, value.clone());
        self.end.set(start + value.len());
        self.data.writes.set(self.data.writes.get() + 1);
        if self.data.mode.get() == EXIT_AFTER_WRITE {
            let probe = self.data.exit_probe.borrow().as_ref().unwrap().clone();
            probe
                .writes
                .store(self.data.writes.get(), Ordering::Release);
            probe
                .first_unit
                .store(u64::from(self.data.text.borrow()[0]), Ordering::Release);
            // 只在隔离子进程的自有线程中注入；写已发生，刻意不执行COM撤销或Rust Drop。
            unsafe {
                windows::Win32::System::Threading::ExitThread(77);
            }
        }
        if self.data.mode.get() == 9 {
            // 故意阻塞当前 STA 的宿主写调用；独立监测仍须在没有消息泵时标记 Unknown。
            std::thread::sleep(Duration::from_millis(650));
            let host = self.data.host.borrow().upgrade().unwrap();
            self.data
                .blocked_detection
                .set(Some(host.watchdog.borrow().as_ref().unwrap().detection()));
            self.data.blocked_quarantine.set(host.quarantined.get());
        }
        if self.data.mode.get() == 3 {
            // 在真正的写入回调内部重入生产失焦，随后观察生产清理的执行顺序。
            self.data.mode.set(0);
            let host = self.data.host.borrow().upgrade().unwrap();
            host.invalidate()?;
        }
        if self.data.mode.get() == 2 {
            return Err(Error::from_hresult(E_FAIL));
        }
        Ok(())
    }
    fn GetFormattedText(
        &self,
        ec: u32,
    ) -> windows_core::Result<windows::Win32::System::Com::IDataObject> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetEmbedded(
        &self,
        ec: u32,
        rguidservice: *const windows_core::GUID,
        riid: *const windows_core::GUID,
    ) -> windows_core::Result<windows_core::IUnknown> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn InsertEmbedded(
        &self,
        ec: u32,
        dwflags: u32,
        pdataobject: windows_core::Ref<windows::Win32::System::Com::IDataObject>,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn ShiftStart(
        &self,
        ec: u32,
        cchreq: i32,
        pcch: *mut i32,
        phalt: *const TF_HALTCOND,
    ) -> windows_core::Result<()> {
        self.start.set((self.start.get() as i32 + cchreq) as usize);
        self.end.set(self.end.get().max(self.start.get()));
        unsafe {
            *pcch = cchreq;
        }
        Ok(())
    }
    fn ShiftEnd(
        &self,
        ec: u32,
        cchreq: i32,
        pcch: *mut i32,
        phalt: *const TF_HALTCOND,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn ShiftStartToRange(
        &self,
        ec: u32,
        prange: windows_core::Ref<ITfRange>,
        apos: TfAnchor,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn ShiftEndToRange(
        &self,
        ec: u32,
        prange: windows_core::Ref<ITfRange>,
        apos: TfAnchor,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn ShiftStartRegion(
        &self,
        ec: u32,
        dir: TfShiftDir,
    ) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn ShiftEndRegion(&self, ec: u32, dir: TfShiftDir) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn IsEmpty(&self, ec: u32) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn Collapse(&self, ec: u32, apos: TfAnchor) -> windows_core::Result<()> {
        if apos == TF_ANCHOR_START {
            self.end.set(self.start.get());
        } else {
            self.start.set(self.end.get());
        }
        Ok(())
    }
    fn IsEqualStart(
        &self,
        ec: u32,
        pwith: windows_core::Ref<ITfRange>,
        apos: TfAnchor,
    ) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn IsEqualEnd(
        &self,
        ec: u32,
        pwith: windows_core::Ref<ITfRange>,
        apos: TfAnchor,
    ) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn CompareStart(
        &self,
        ec: u32,
        pwith: windows_core::Ref<ITfRange>,
        apos: TfAnchor,
    ) -> windows_core::Result<i32> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn CompareEnd(
        &self,
        ec: u32,
        pwith: windows_core::Ref<ITfRange>,
        apos: TfAnchor,
    ) -> windows_core::Result<i32> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn AdjustForInsert(&self, ec: u32, cchinsert: u32) -> windows_core::Result<windows_core::BOOL> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetGravity(
        &self,
        pgstart: *mut TfGravity,
        pgend: *mut TfGravity,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn SetGravity(&self, ec: u32, gstart: TfGravity, gend: TfGravity) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn Clone(&self) -> windows_core::Result<ITfRange> {
        Ok(Range {
            data: self.data.clone(),
            start: Cell::new(self.start.get().min(self.data.text.borrow().len())),
            end: Cell::new(self.end.get().min(self.data.text.borrow().len())),
        }
        .into())
    }
    fn GetContext(&self) -> windows_core::Result<ITfContext> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
}
impl ITfContext_Impl for Context_Impl {
    fn RequestEditSession(
        &self,
        tid: u32,
        pes: windows_core::Ref<ITfEditSession>,
        dwflags: TF_CONTEXT_EDIT_CONTEXT_FLAGS,
    ) -> windows_core::Result<windows_core::HRESULT> {
        if dwflags.contains(TF_ES_SYNC) {
            if self.data.mode.get() == 7 {
                return Ok(TS_E_SYNCHRONOUS);
            }
            // 故意保留已被拒绝的同步对象，验证违约晚到回调也不能二次执行。
            if self.data.mode.get() == 8 {
                self.data
                    .pending
                    .borrow_mut()
                    .push_back(pes.as_ref().unwrap().clone());
                return Ok(TS_E_SYNCHRONOUS);
            }
            return Ok(unsafe { pes.as_ref().unwrap().DoEditSession(7) }
                .map_or_else(|e| e.code(), |_| S_OK));
        }
        self.data
            .pending
            .borrow_mut()
            .push_back(pes.as_ref().unwrap().clone());
        Ok(TS_S_ASYNC)
    }
    fn InWriteSession(&self, tid: u32) -> windows_core::Result<windows_core::BOOL> {
        self.data.calls.borrow_mut().push("InWriteSession");
        match self.data.write_owner.get() {
            0 => Ok(false.into()),
            1 => Ok(true.into()),
            _ => Err(Error::from_hresult(E_FAIL)),
        }
    }
    fn GetSelection(
        &self,
        ec: u32,
        ulindex: u32,
        ulcount: u32,
        pselection: *mut TF_SELECTION,
        pcfetched: *mut u32,
    ) -> windows_core::Result<()> {
        unsafe {
            *pselection = TF_SELECTION {
                range: std::mem::ManuallyDrop::new(Some(
                    match self.data.selection.borrow().as_ref() {
                        Some(selection) => selection.Clone()?,
                        None => range(&self.data),
                    },
                )),
                style: TF_SELECTIONSTYLE::default(),
            };
            *pcfetched = 1;
        }
        Ok(())
    }
    fn SetSelection(
        &self,
        ec: u32,
        ulcount: u32,
        pselection: *const TF_SELECTION,
    ) -> windows_core::Result<()> {
        if self.data.mode.get() == 5 {
            return Err(Error::from_hresult(E_FAIL));
        }
        // 安全：调用期间选择结构及 range 引用有效，复制锚点而非保留借用。
        *self.data.selection.borrow_mut() = unsafe {
            (*pselection)
                .range
                .as_ref()
                .map(|range| range.Clone())
                .transpose()?
        };
        Ok(())
    }
    fn GetStart(&self, ec: u32) -> windows_core::Result<ITfRange> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetEnd(&self, ec: u32) -> windows_core::Result<ITfRange> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetActiveView(&self) -> windows_core::Result<ITfContextView> {
        Ok(Layout.into())
    }
    fn EnumViews(&self) -> windows_core::Result<IEnumTfContextViews> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetStatus(&self) -> windows_core::Result<TS_STATUS> {
        if self.data.mode.get() == 6 {
            return Err(Error::from_hresult(E_FAIL));
        }
        Ok(TS_STATUS::default())
    }
    fn GetProperty(
        &self,
        guidprop: *const windows_core::GUID,
    ) -> windows_core::Result<ITfProperty> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetAppProperty(
        &self,
        guidprop: *const windows_core::GUID,
    ) -> windows_core::Result<ITfReadOnlyProperty> {
        Ok(Property.into())
    }
    fn TrackProperties(
        &self,
        prgprop: *const *const windows_core::GUID,
        cprop: u32,
        prgappprop: *const *const windows_core::GUID,
        cappprop: u32,
    ) -> windows_core::Result<ITfReadOnlyProperty> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn EnumProperties(&self) -> windows_core::Result<IEnumTfProperties> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetDocumentMgr(&self) -> windows_core::Result<ITfDocumentMgr> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn CreateRangeBackup(
        &self,
        ec: u32,
        prange: windows_core::Ref<ITfRange>,
    ) -> windows_core::Result<ITfRangeBackup> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
}
impl ITfContextComposition_Impl for Context_Impl {
    fn StartComposition(
        &self,
        ecwrite: u32,
        pcompositionrange: windows_core::Ref<ITfRange>,
        psink: windows_core::Ref<ITfCompositionSink>,
    ) -> windows_core::Result<ITfComposition> {
        Ok(Composition {
            data: self.data.clone(),
            range: pcompositionrange.as_ref().unwrap().clone(),
        }
        .into())
    }
    fn EnumCompositions(&self) -> windows_core::Result<IEnumITfCompositionView> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn FindComposition(
        &self,
        ecread: u32,
        ptestrange: windows_core::Ref<ITfRange>,
    ) -> windows_core::Result<IEnumITfCompositionView> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn TakeOwnership(
        &self,
        ecwrite: u32,
        pcomposition: windows_core::Ref<ITfCompositionView>,
        psink: windows_core::Ref<ITfCompositionSink>,
    ) -> windows_core::Result<ITfComposition> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
}
impl ITfComposition_Impl for Composition_Impl {
    fn GetRange(&self) -> windows_core::Result<ITfRange> {
        self.data.calls.borrow_mut().push("GetRange");
        Ok(self.range.clone())
    }
    fn ShiftStart(
        &self,
        ecwrite: u32,
        pnewstart: windows_core::Ref<ITfRange>,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn ShiftEnd(
        &self,
        ecwrite: u32,
        pnewend: windows_core::Ref<ITfRange>,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn EndComposition(&self, ecwrite: u32) -> windows_core::Result<()> {
        self.data.calls.borrow_mut().push("EndComposition");
        self.data.endings.set(self.data.endings.get() + 1);
        if self.data.mode.get() == 4 {
            return Err(Error::from_hresult(E_FAIL));
        }
        if self.data.mode.get() == 10 {
            // 故意在旧 EndComposition 内切换 context 并安装新组合，旧事务不得取走新资源。
            self.data.mode.set(0);
            let host = self.data.host.borrow().upgrade().unwrap();
            host.invalidate()?;
            self.data
                .text
                .borrow_mut()
                .extend("新上下文文本".encode_utf16());
            *host.context.borrow_mut() = Some(
                Context {
                    data: self.data.clone(),
                }
                .into(),
            );
            *host.composition.borrow_mut() = Some(
                Composition {
                    data: self.data.clone(),
                    range: range(&self.data),
                }
                .into(),
            );
        }
        Ok(())
    }
}
impl ITfReadOnlyProperty_Impl for Property_Impl {
    fn GetType(&self) -> windows_core::Result<windows_core::GUID> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn EnumRanges(
        &self,
        ec: u32,
        ppenum: windows_core::OutRef<IEnumTfRanges>,
        ptargetrange: windows_core::Ref<ITfRange>,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetValue(
        &self,
        ec: u32,
        prange: windows_core::Ref<ITfRange>,
    ) -> windows_core::Result<windows::Win32::System::Variant::VARIANT> {
        Ok(VARIANT::default())
    }
    fn GetContext(&self) -> windows_core::Result<ITfContext> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
}
impl ITfContextView_Impl for Layout_Impl {
    fn GetRangeFromPoint(
        &self,
        ec: u32,
        ppt: *const windows::Win32::Foundation::POINT,
        dwflags: u32,
    ) -> windows_core::Result<ITfRange> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetTextExt(
        &self,
        ec: u32,
        prange: windows_core::Ref<ITfRange>,
        prc: *mut windows::Win32::Foundation::RECT,
        pfclipped: *mut windows_core::BOOL,
    ) -> windows_core::Result<()> {
        Err(Error::from_hresult(TS_E_NOLAYOUT))
    }
    fn GetScreenExt(&self) -> windows_core::Result<windows::Win32::Foundation::RECT> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetWnd(&self) -> windows_core::Result<windows::Win32::Foundation::HWND> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
}

#[test]
fn review_production_commit_stages() {
    // 0成功；1写入调用拒绝；2写入后失败；4结束组合失败；5设置选区失败；6写入前状态失败。
    for mode in [0u8, 1, 2, 4, 5, 6] {
        let (host, data, context) = fixture();
        seed_composition(&host, &data);
        let frame = host.engine.borrow().frame();
        let expected = frame.candidates[0].text.clone();
        data.mode.set(mode);
        host.queue.borrow_mut().push_back(Operation::Select(
            frame.revision,
            frame.candidates[0].id.clone(),
        ));
        let result = execute(&host, &context);
        println!(
            "stage mode={mode}: result={:?} writes={} text={:?} quarantined={} unknown={} state={:?} calls={:?}",
            result.as_ref().map_err(|e| e.code()),
            data.writes.get(),
            text(&data),
            host.quarantined.get(),
            host.unknown_text.get(),
            host.engine.borrow().frame().state,
            data.calls.borrow()
        );
        if mode == 0 {
            assert!(result.is_ok());
            assert_eq!(text(&data), expected);
            assert!(!host.unknown_text.get());
        } else {
            assert!(result.is_err());
        }
        if [1, 2, 4, 5].contains(&mode) {
            assert!(host.unknown_text.get());
        }
        if mode == 6 {
            assert!(!host.unknown_text.get());
        }
        if mode == 1 || mode == 6 {
            assert_eq!(text(&data), "ni");
            assert_eq!(data.writes.get(), 0);
        }
        if [0, 2, 4, 5].contains(&mode) {
            assert_eq!(text(&data), expected);
            assert_eq!(data.writes.get(), 1);
        }
        if [1, 2, 4, 5].contains(&mode) {
            assert!(matches!(
                host.engine.borrow().key_disposition(Key::Space),
                KeyDisposition::Blocked(cloudleaf_types::ErrorCode::CommitPending)
            ));
        }
        if mode == 6 {
            assert!(matches!(
                host.engine.borrow().key_disposition(Key::Space),
                KeyDisposition::Blocked(cloudleaf_types::ErrorCode::CommitFailed)
            ));
        }
    }
}
#[test]
fn review_settext_reentrant_focus_preserves_written_commit() {
    let (host, data, context) = fixture();
    seed_composition(&host, &data);
    let frame = host.engine.borrow().frame();
    let expected = frame.candidates[0].text.clone();
    data.mode.set(3);
    host.queue.borrow_mut().push_back(Operation::Select(
        frame.revision,
        frame.candidates[0].id.clone(),
    ));
    let _ = execute(&host, &context);
    drain(&data);
    println!(
        "focus hook: text={:?} writes={} endings={} calls={:?}",
        text(&data),
        data.writes.get(),
        data.endings.get(),
        data.calls.borrow()
    );
    assert_eq!(text(&data), expected);
    assert_eq!(data.writes.get(), 1);
    assert_eq!(data.endings.get(), 1);
}
#[test]
fn review_duplicate_keyedit_executes_once() {
    let (host, data, context) = fixture();
    let ran = Rc::new(Cell::new(false));
    let edit: ITfEditSession = KeyEdit {
        host: host.clone(),
        epoch: host.epoch.get(),
        context,
        action: Operation::Key(Key::Char('n')),
        executed: ran.clone(),
        called: Rc::new(Cell::new(false)),
    }
    .into();
    unsafe {
        let _ = edit.DoEditSession(7);
        let _ = edit.DoEditSession(7);
    }
    println!(
        "duplicate same KeyEdit: text={:?} preedit={:?} writes={}",
        text(&data),
        host.engine.borrow().frame().preedit,
        data.writes.get()
    );
    assert_eq!(host.engine.borrow().frame().preedit, "n");
    assert!(!host.quarantined.get());
}
#[test]
fn review_distinct_keyedits_preserve_legal_auto_repeat() {
    let (host, data, context) = fixture();
    for _ in 0..3 {
        assert!(host.key(Key::Char('n')).unwrap());
        assert!(!host.quarantined.get());
    }
    assert_eq!(host.engine.borrow().frame().preedit, "nnn");
    assert_eq!(text(&data), "nnn");
    assert_eq!(data.writes.get(), 3);
}
#[test]
fn review_candidate_sync_lock_rejection_preserves_frame_and_original_key() {
    let (host, data, _) = fixture();
    seed_composition(&host, &data);
    let frame = host.engine.borrow().frame();
    data.mode.set(7);
    assert!(
        host.select(frame.revision, frame.candidates[0].id.clone())
            .is_err()
    );
    assert!(data.pending.borrow().is_empty());
    assert_eq!(host.engine.borrow().frame(), frame);
    assert!(host.disposition(Key::Char('x')).unwrap());
    println!(
        "candidate refused lock: preserved frame, no queued commit, native x still admissible"
    );
}
#[test]
fn review_rejected_sync_callback_cannot_later_consume_native_key_or_candidate() {
    for candidate in [false, true] {
        let (host, data, _) = fixture();
        seed_composition(&host, &data);
        let before = host.engine.borrow().frame();
        data.mode.set(8);
        if candidate {
            assert!(
                host.select(before.revision, before.candidates[0].id.clone())
                    .is_err()
            );
        } else {
            assert!(!host.key(Key::Char('x')).unwrap());
        }
        assert_eq!(data.pending.borrow().len(), 1);
        data.mode.set(0);
        drain(&data);
        assert_eq!(host.engine.borrow().frame(), before);
        assert_eq!(text(&data), "ni");
        assert_eq!(data.writes.get(), 0);
    }
}
#[test]
fn review_rejected_sync_cleanup_cannot_erase_later_native_text() {
    let (host, data, _) = fixture();
    seed_composition(&host, &data);
    data.mode.set(8);
    host.invalidate().unwrap();
    assert_eq!(data.pending.borrow().len(), 2);
    data.text.borrow_mut().push('x' as u16);
    data.mode.set(0);
    drain(&data);
    assert_eq!(text(&data), "nix");
    assert_eq!(data.writes.get(), 0);
    assert_eq!(data.endings.get(), 1);
}
#[test]
fn review_duplicate_edit_and_stale_candidate_do_not_repeat_commit_or_ack() {
    let (host, data, context) = fixture();
    seed_composition(&host, &data);
    let frame = host.engine.borrow().frame();
    let stale_revision = frame.revision;
    let id = frame.candidates[0].id.clone();
    host.queue
        .borrow_mut()
        .push_back(Operation::Select(frame.revision, id.clone()));
    *host.pending_layout.borrow_mut() = Some((42, Instant::now()));
    let edit: ITfEditSession = Edit {
        host: host.clone(),
        epoch: host.epoch.get(),
        context,
        operation: 42,
    }
    .into();
    unsafe {
        edit.DoEditSession(7).unwrap();
        edit.DoEditSession(7).unwrap();
    }
    let committed = host.engine.borrow().frame();
    assert_eq!(data.writes.get(), 1);
    assert_eq!(data.endings.get(), 1);
    assert!(host.pending_layout.borrow().is_none());
    assert!(host.select(stale_revision, id).is_err());
    assert!(!host.quarantined.get());
    assert_eq!(host.engine.borrow().frame(), committed);
    assert_eq!(data.writes.get(), 1);
}
#[test]
fn review_expired_layout_retains_owned_queue_without_writing() {
    let (host, data, _) = fixture();
    seed_composition(&host, &data);
    host.queue.borrow_mut().push_back(Operation::Refresh);
    host.schedule().unwrap();
    let operation = host.pending_layout.borrow().as_ref().unwrap().0;
    *host.pending_layout.borrow_mut() = Some((operation, Instant::now() - Duration::from_secs(1)));
    drain(&data);
    assert!(host.quarantined.get());
    assert!(!host.scheduled.get());
    assert!(host.pending_layout.borrow().is_none());
    assert_eq!(host.queue.borrow().len(), 1);
    assert_eq!(data.writes.get(), 0);
    assert_eq!(text(&data), "ni");
}
#[test]
fn review_async_callback_keeps_host_alive_until_callback_is_released() {
    let (host, data, _) = fixture();
    let weak = Rc::downgrade(&host);
    host.schedule().unwrap();
    drop(host);
    assert!(weak.upgrade().is_some());
    drain(&data);
    assert!(weak.upgrade().is_none());
}

#[test]
#[ignore = "模拟 COM 机制诊断；显式使用 release 与 ignored 参数运行，不是真实 Windows 应用基准"]
fn simulated_com_diagnostic_benchmark() {
    const WARMUP: usize = 100;
    const SAMPLES: usize = 1000;
    for commit in [false, true] {
        let mut samples = Vec::with_capacity(SAMPLES);
        for iteration in 0..WARMUP + SAMPLES {
            let (host, data, _) = fixture();
            let selection = if commit {
                seed_composition(&host, &data);
                let frame = host.engine.borrow().frame();
                Some((frame.revision, frame.candidates[0].id.clone()))
            } else {
                None
            };
            // 计时仅涵盖生产原生事务；词典构造、初始组合布置与销毁不计入样本。
            let started = Instant::now();
            match selection {
                Some((revision, id)) => host.select(revision, id).unwrap(),
                None => assert!(host.key(Key::Char('n')).unwrap()),
            }
            let elapsed = started.elapsed().as_nanos();
            assert_eq!(data.writes.get(), 1);
            assert!(!host.quarantined.get());
            if iteration >= WARMUP {
                samples.push(elapsed);
            }
        }
        samples.sort_unstable();
        let percentile = |percent: usize| samples[(SAMPLES * percent).div_ceil(100) - 1];
        println!(
            "{{\"benchmark\":\"simulated-com-production-transaction\",\"real_windows_application\":false,\"production_tsf_performance\":false,\"fixture\":\"prototype64/VT_EMPTY_scope/TS_E_NOLAYOUT\",\"path\":\"{}\",\"warmup\":{},\"samples\":{},\"unit\":\"ns\",\"p50\":{},\"p95\":{},\"p99\":{},\"max\":{}}}",
            if commit {
                "candidate_commit"
            } else {
                "keyedit"
            },
            WARMUP,
            SAMPLES,
            percentile(50),
            percentile(95),
            percentile(99),
            samples[SAMPLES - 1],
        );
    }
}
