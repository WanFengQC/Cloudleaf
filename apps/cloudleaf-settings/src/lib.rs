//! 输入偏好的序列化与后台存储。按键路径只使用已加载配置。
mod input;
mod runtime;
mod storage;

pub use input::{
    InputFuzzyPair, InputPreferences, InputPreferencesRepository, InputScheme, InputSettingsError,
    InputSettingsStatus, LoadedInputPreferences,
};
pub use runtime::{
    RuntimeSettingsReason, RuntimeSettingsReceipt, RuntimeSettingsStatus, RuntimeSettingsWatch,
    read_runtime_receipts,
};
pub use storage::{read_bounded, write_atomic};
