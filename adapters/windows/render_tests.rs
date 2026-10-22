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
    (host, data, context)
}
fn seed_composition(host: &Shared, data: &Rc<Data>) {
    // 直接布置初始组合用于隔离写入阶段；不把这一步算作原生按键成功证据。
    for c in ['n', 'i'] {
        host.engine
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
        let start = self.start.get();
        let end = self.end.get().min(self.data.text.borrow().len());
        self.data
            .text
            .borrow_mut()
            .splice(start..end, value.clone());
        self.end.set(start + value.len());
        self.data.writes.set(self.data.writes.get() + 1);
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
            start: Cell::new(self.start.get()),
            end: Cell::new(self.end.get()),
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
        Err(Error::from_hresult(E_NOTIMPL))
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
                range: std::mem::ManuallyDrop::new(Some(range(&self.data))),
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
