use crate::{ErrorKind, SearchError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// 仅传递取消意图，不同步结果或输入上下文；取消后不能复用为新请求。
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub(crate) fn check(&self) -> Result<(), SearchError> {
        if self.is_cancelled() {
            Err(SearchError::new(ErrorKind::Cancelled, "搜索已取消"))
        } else {
            Ok(())
        }
    }
}
