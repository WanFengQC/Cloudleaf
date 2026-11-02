//! owner 的真实线程句柄只证明退出，不转移 COM 或窗口所有权。
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::Arc,
};
use windows::Win32::{
    Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        GetCurrentProcess, GetCurrentThread, GetCurrentThreadId, WaitForSingleObject,
    },
};
use windows_core::Result;

#[derive(Debug)]
pub(crate) struct OwnerThreadProof {
    owner: u32,
    thread: OwnedHandle,
}

impl OwnerThreadProof {
    pub(crate) fn capture_current() -> Result<Arc<Self>> {
        let mut duplicated = HANDLE::default();
        // 安全：仅在正常 owner 入口复制自己的伪句柄；目标独占槽有效且不继承。
        // 真实句柄持续指向原线程对象，不通过可能复用的线程 ID 重新打开。
        let owner = unsafe {
            let process = GetCurrentProcess();
            DuplicateHandle(
                process,
                GetCurrentThread(),
                process,
                &mut duplicated,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )?;
            GetCurrentThreadId()
        };
        // 安全：DuplicateHandle 成功返回的独占真实句柄由 OwnedHandle 最后关闭一次。
        let thread = unsafe { OwnedHandle::from_raw_handle(duplicated.0) };
        Ok(Arc::new(Self { owner, thread }))
    }

    pub(crate) fn owner_id(&self) -> u32 {
        self.owner
    }

    pub(crate) fn is_owner(&self) -> bool {
        // 安全：只读取当前 ID；即使 ID 被复用，原线程对象已退出也不能认成 owner。
        // 等待失败也不能授权当前线程，必须明确观察到原线程仍活跃。
        unsafe {
            GetCurrentThreadId() == self.owner
                && WaitForSingleObject(HANDLE(self.thread.as_raw_handle()), 0) == WAIT_TIMEOUT
        }
    }

    pub(crate) fn exited(&self) -> bool {
        // 安全：OwnedHandle 在整个调用期间有效；只零等待，不 join、不触及 User32/COM。
        // WAIT_TIMEOUT 和 WAIT_FAILED 均无法证明退出，不能据此退还资源或卸载模块。
        unsafe { WaitForSingleObject(HANDLE(self.thread.as_raw_handle()), 0) == WAIT_OBJECT_0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn current_thread_is_owner_but_cross_thread_observer_is_not() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<OwnerThreadProof>();
        let proof = OwnerThreadProof::capture_current().unwrap();
        // 安全：测试只读取当前线程标识。
        assert_eq!(proof.owner_id(), unsafe { GetCurrentThreadId() });
        assert!(proof.is_owner());
        assert!(!proof.exited());
        let observer = proof.clone();
        thread::spawn(move || {
            assert!(!observer.is_owner());
            assert!(!observer.exited());
        })
        .join()
        .unwrap();
        assert!(proof.is_owner());
        assert!(!proof.exited());
    }

    #[test]
    fn actual_wait_access_failure_proves_neither_owner_nor_exit() {
        use windows::Win32::{
            Foundation::{DUPLICATE_HANDLE_OPTIONS, WAIT_FAILED},
            System::Threading::THREAD_QUERY_LIMITED_INFORMATION,
        };
        let mut restricted = HANDLE::default();
        // 安全：自己的线程复制为只查询、不含 SYNCHRONIZE 的真实句柄，故意触发实际等待拒绝。
        let owner = unsafe {
            let process = GetCurrentProcess();
            DuplicateHandle(
                process,
                GetCurrentThread(),
                process,
                &mut restricted,
                THREAD_QUERY_LIMITED_INFORMATION.0,
                false,
                DUPLICATE_HANDLE_OPTIONS(0),
            )
            .unwrap();
            GetCurrentThreadId()
        };
        // 安全：只查询权限的句柄仍有效且只由本测试包装负责关闭一次。
        let proof = OwnerThreadProof {
            owner,
            thread: unsafe { OwnedHandle::from_raw_handle(restricted.0) },
        };
        // 安全：句柄有效但没有等待权限，验证真实 Win32 WAIT_FAILED 而非逻辑模拟。
        assert_eq!(
            unsafe { WaitForSingleObject(HANDLE(proof.thread.as_raw_handle()), 0) },
            WAIT_FAILED
        );
        assert!(!proof.is_owner());
        assert!(!proof.exited());
    }

    #[test]
    fn duplicate_proves_original_os_thread_exit_after_join() {
        let (publish, receive) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(0);
        let owner = thread::spawn(move || {
            let proof = OwnerThreadProof::capture_current().unwrap();
            assert!(proof.is_owner());
            publish.send(proof).unwrap();
            wait.recv().unwrap();
        });
        let proof = receive.recv().unwrap();
        assert!(!proof.is_owner());
        assert!(!proof.exited());
        release.send(()).unwrap();
        owner.join().unwrap();
        assert!(proof.exited());
        assert!(!proof.is_owner());
    }

    #[test]
    fn duplicate_remains_valid_after_rust_join_handle_is_dropped() {
        let (publish, receive) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(0);
        let owner = thread::spawn(move || {
            publish
                .send(OwnerThreadProof::capture_current().unwrap())
                .unwrap();
            wait.recv().unwrap();
        });
        let proof = receive.recv().unwrap();
        drop(owner);
        assert!(!proof.exited());
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !proof.exited() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(proof.exited());
    }

    #[test]
    fn repeated_owner_receipts_keep_live_and_exited_proofs_distinct() {
        let current = OwnerThreadProof::capture_current().unwrap();
        for _ in 0..128 {
            let proof = thread::spawn(|| OwnerThreadProof::capture_current().unwrap())
                .join()
                .unwrap();
            assert!(proof.exited());
            assert!(!proof.is_owner());
            assert!(!current.exited());
            assert!(current.is_owner());
            let last = Arc::downgrade(&proof);
            let retained = proof.clone();
            drop(proof);
            assert!(last.upgrade().is_some());
            drop(retained);
            assert!(last.upgrade().is_none());
        }
    }
}
