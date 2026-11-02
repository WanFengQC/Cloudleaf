//! 使用本地固定模型的离线整句推理，不包含网络或密钥访问。

mod manager;
mod marian;

pub use manager::*;
pub use marian::*;
