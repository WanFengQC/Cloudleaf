//! 真正调用本适配器 COM 回调的故障注入；不冒充 Windows 应用验收。
use super::*;
use windows::Win32::Foundation::{E_NOTIMPL, S_OK};
use windows_core::{BOOL, GUID, HRESULT, Ref};

#[implement(ITfContext, Agile = false)]
struct FaultContext {
    mode: u8,
    readonly: bool,
}
impl ITfContext_Impl for FaultContext_Impl {
    fn RequestEditSession(
        &self,
        _tid: u32,
        edit: Ref<ITfEditSession>,
        flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS,
    ) -> Result<HRESULT> {
        if self.mode < 3 {
            assert!(flags.contains(TF_ES_SYNC));
        }
        match self.mode {
            0 => Err(Error::from_hresult(E_FAIL)),
            1 => Ok(TS_E_SYNCHRONOUS),
            _ => {
                // 安全：本测试宿主显式授予有效的测试编辑 cookie。
                Ok(unsafe { edit.as_ref().unwrap().DoEditSession(7) }
                    .map_or_else(|e| e.code(), |_| S_OK))
            }
        }
    }
    fn InWriteSession(&self, _tid: u32) -> Result<BOOL> {
        Ok(false.into())
    }
    fn GetStatus(&self) -> Result<TS_STATUS> {
        Ok(TS_STATUS {
            dwDynamicFlags: if self.readonly { TS_SD_READONLY } else { 0 },
            ..Default::default()
        })
    }
    fn GetSelection(
        &self,
        _ec: u32,
        _index: u32,
        _count: u32,
        _selection: *mut TF_SELECTION,
        _fetched: *mut u32,
    ) -> Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn SetSelection(&self, _ec: u32, _count: u32, _selection: *const TF_SELECTION) -> Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetStart(&self, _ec: u32) -> Result<ITfRange> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetEnd(&self, _ec: u32) -> Result<ITfRange> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetActiveView(&self) -> Result<ITfContextView> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn EnumViews(&self) -> Result<IEnumTfContextViews> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetProperty(&self, _guid: *const GUID) -> Result<ITfProperty> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetAppProperty(&self, _guid: *const GUID) -> Result<ITfReadOnlyProperty> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn TrackProperties(
        &self,
        _properties: *const *const GUID,
        _count: u32,
        _app: *const *const GUID,
        _app_count: u32,
    ) -> Result<ITfReadOnlyProperty> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn EnumProperties(&self) -> Result<IEnumTfProperties> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn CreateRangeBackup(&self, _ec: u32, _range: Ref<ITfRange>) -> Result<ITfRangeBackup> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
}

pub(crate) fn host(mode: u8, readonly: bool) -> Shared {
    let host = Host::new().unwrap();
    *host.context.borrow_mut() = Some(FaultContext { mode, readonly }.into());
    host.engine.borrow_mut().handle(InputEvent::Activate);
    host
}

#[test]
fn rejected_sync_lock_preserves_core_queue_and_original_key() {
    for mode in [0, 1] {
        let host = host(mode, false);
        let before = host.engine.borrow().frame();
        assert!(!host.key(Key::Char('n')).unwrap());
        assert_eq!(before, host.engine.borrow().frame());
        assert!(host.queue.borrow().is_empty());
        assert!(!host.scheduled.get());
        assert!(!host.quarantined.get());
    }
}

#[test]
fn readonly_write_callback_does_not_consume_or_mutate_core() {
    let host = host(2, true);
    let before = host.engine.borrow().frame();
    assert!(!host.key(Key::Char('n')).unwrap());
    assert_eq!(before, host.engine.borrow().frame());
    assert!(host.queue.borrow().is_empty());
}

#[test]
fn missing_input_scope_fails_closed_without_consuming_key() {
    let host = host(2, false);
    let before = host.engine.borrow().frame();
    assert!(!host.key(Key::Char('n')).unwrap());
    assert_eq!(before, host.engine.borrow().frame());
    assert!(!host.quarantined.get());
}

#[test]
fn late_edit_does_not_clear_new_epoch_queue_or_schedule() {
    let host = host(0, false);
    let context = host.context.borrow().clone().unwrap();
    let old: ITfEditSession = Edit {
        host: host.clone(),
        epoch: host.epoch.get(),
        context,
        operation: 1,
    }
    .into();
    host.invalidate().unwrap();
    host.engine.borrow_mut().handle(InputEvent::Activate);
    host.queue.borrow_mut().push_back(Operation::Cancel);
    host.scheduled.set(true);
    let before = host.engine.borrow().frame();
    // 安全：直接触发已失效回调，验证其在访问宿主文本之前被拒绝。
    unsafe {
        old.DoEditSession(7).unwrap();
    }
    assert_eq!(before, host.engine.borrow().frame());
    assert_eq!(host.queue.borrow().len(), 1);
    assert!(host.scheduled.get());
}

#[test]
fn rejected_candidate_lock_keeps_revision_text_and_candidates_for_retry() {
    let host = host(1, false);
    host.engine
        .borrow_mut()
        .handle(InputEvent::Key(Key::Char('n')));
    let frame = host
        .engine
        .borrow_mut()
        .handle(InputEvent::Key(Key::Char('i')))
        .frame;
    let selected = frame.candidates[0].id.clone();
    assert!(host.select(frame.revision, selected).is_err());
    assert_eq!(frame, host.engine.borrow().frame());
    assert!(host.queue.borrow().is_empty());
    assert!(!host.scheduled.get());
}

#[test]
fn expired_layout_callback_quarantines_without_running_queued_operations() {
    let host = host(0, false);
    let context = host.context.borrow().clone().unwrap();
    *host.pending_layout.borrow_mut() = Some((8, Instant::now() - Duration::from_secs(1)));
    host.scheduled.set(true);
    host.queue.borrow_mut().push_back(Operation::Cancel);
    let before = host.engine.borrow().frame();
    let edit: ITfEditSession = Edit {
        host: host.clone(),
        epoch: host.epoch.get(),
        context,
        operation: 8,
    }
    .into();
    // 安全：过期回调不得触及上下文，只转入显式隔离状态。
    assert!(unsafe { edit.DoEditSession(7) }.is_err());
    assert_eq!(before, host.engine.borrow().frame());
    assert!(host.quarantined.get());
    assert_eq!(host.queue.borrow().len(), 1);
}

#[test]
fn duplicate_layout_callback_is_not_a_second_dispatch() {
    let host = host(0, false);
    let context = host.context.borrow().clone().unwrap();
    *host.pending_layout.borrow_mut() = Some((9, Instant::now()));
    let edit: ITfEditSession = Edit {
        host: host.clone(),
        epoch: host.epoch.get(),
        context,
        operation: 9,
    }
    .into();
    // 安全：空布局队列首次回调完成，重复通知不能清新排队操作。
    unsafe {
        edit.DoEditSession(7).unwrap();
    }
    host.queue.borrow_mut().push_back(Operation::Cancel);
    unsafe {
        edit.DoEditSession(7).unwrap();
    }
    assert_eq!(host.queue.borrow().len(), 1);
}

#[test]
fn reentrant_candidate_abort_is_queued_even_while_rendering() {
    let host = host(0, false);
    host.scheduled.set(true);
    host.cancel().unwrap();
    assert!(matches!(
        host.queue.borrow().front(),
        Some(Operation::Cancel)
    ));
}

#[implement(ITfComposition, Agile = false)]
struct UnknownComposition {
    range_reads: Rc<Cell<usize>>,
    endings: Rc<Cell<usize>>,
}
impl ITfComposition_Impl for UnknownComposition_Impl {
    fn GetRange(&self) -> Result<ITfRange> {
        self.range_reads.set(self.range_reads.get() + 1);
        Err(Error::from_hresult(E_FAIL))
    }
    fn ShiftStart(&self, _ec: u32, _range: Ref<ITfRange>) -> Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn ShiftEnd(&self, _ec: u32, _range: Ref<ITfRange>) -> Result<()> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn EndComposition(&self, _ec: u32) -> Result<()> {
        self.endings.set(self.endings.get() + 1);
        Ok(())
    }
}

#[test]
fn unknown_cleanup_never_obtains_or_erases_text_range() {
    let range_reads = Rc::new(Cell::new(0));
    let endings = Rc::new(Cell::new(0));
    let cleanup: ITfEditSession = Cleanup {
        composition: UnknownComposition {
            range_reads: range_reads.clone(),
            endings: endings.clone(),
        }
        .into(),
        erase: false,
        _lease: super::super::service::Lease::new(),
    }
    .into();
    // 安全：测试直接调用清理回调，不注册或激活任何系统服务。
    unsafe {
        cleanup.DoEditSession(7).unwrap();
    }
    assert_eq!(range_reads.get(), 0);
    assert_eq!(endings.get(), 1);
}

#[test]
fn unknown_marker_is_consumed_even_without_composition() {
    let host = host(0, false);
    host.unknown_text.set(true);
    host.invalidate().unwrap();
    assert!(!host.unknown_text.get());
    assert!(!host.quarantined.get());
}

#[test]
fn focus_loss_during_uncertain_commit_only_ends_old_composition() {
    let host = host(3, false);
    let reads = Rc::new(Cell::new(0));
    let endings = Rc::new(Cell::new(0));
    *host.composition.borrow_mut() = Some(
        UnknownComposition {
            range_reads: reads.clone(),
            endings: endings.clone(),
        }
        .into(),
    );
    host.unknown_text.set(true);
    host.invalidate().unwrap();
    assert_eq!(reads.get(), 0);
    assert_eq!(endings.get(), 1);
    assert!(host.composition.borrow().is_none());
    assert!(host.context.borrow().is_none());
    assert!(!host.unknown_text.get());
}
