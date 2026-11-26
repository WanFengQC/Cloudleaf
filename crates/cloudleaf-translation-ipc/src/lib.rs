//! 私有管道上的离线翻译协议。输入侧不依赖联网、凭据、模型或翻译执行库。

mod client;
mod wire;

pub use client::{WorkerClient, WorkerConfig, WorkerEvent, WorkerScheduling, WorkerState};
pub use wire::*;
