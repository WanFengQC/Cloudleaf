//! 独立于按键核心的翻译请求、后台任务及提供者契约。

mod dictionary;
mod interaction;
mod offline;
mod online;
mod provider;
mod scheduler;
mod secret;
mod types;

pub use dictionary::*;
pub use interaction::*;
pub use offline::*;
pub use online::*;
pub use provider::*;
pub use scheduler::*;
pub use secret::*;
pub use types::*;
