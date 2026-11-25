use crate::TranslationError;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;
use zeroize::Zeroize;

pub struct Secret(Vec<u8>);

impl Secret {
    pub fn new(mut value: String) -> Result<Self, TranslationError> {
        if value.is_empty() || value.len() > 8192 || value.chars().any(char::is_control) {
            value.zeroize();
            return Err(TranslationError::InvalidConfig);
        }
        Ok(Self(value.into_bytes()))
    }

    pub fn expose(&self) -> &str {
        std::str::from_utf8(&self.0).expect("Secret只接收有效UTF-8")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // 只清理自己拥有的缓冲，不承诺HTTP栈临时副本也已清零。
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

pub trait SecretStore: Send + Sync + 'static {
    /// 平台存储读取只在后台执行，不记录引用或密钥正文。
    fn get(&self, reference: &str) -> Result<Secret, TranslationError>;
}

#[derive(Default)]
pub struct SessionSecretStore {
    secrets: Mutex<BTreeMap<String, Secret>>,
}

impl SessionSecretStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, reference: String, secret: Secret) -> Result<(), TranslationError> {
        if reference.is_empty()
            || reference.len() > 128
            || !reference
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || b"-_.:".contains(&value))
        {
            return Err(TranslationError::InvalidConfig);
        }
        let mut secrets = self
            .secrets
            .lock()
            .map_err(|_| TranslationError::SecretStoreUnavailable)?;
        if secrets.len() >= 32 && !secrets.contains_key(&reference) {
            return Err(TranslationError::Busy);
        }
        secrets.insert(reference, secret);
        Ok(())
    }

    pub fn remove(&self, reference: &str) -> Result<bool, TranslationError> {
        Ok(self
            .secrets
            .lock()
            .map_err(|_| TranslationError::SecretStoreUnavailable)?
            .remove(reference)
            .is_some())
    }

    pub fn clear(&self) -> Result<(), TranslationError> {
        self.secrets
            .lock()
            .map_err(|_| TranslationError::SecretStoreUnavailable)?
            .clear();
        Ok(())
    }
}

impl SecretStore for SessionSecretStore {
    fn get(&self, reference: &str) -> Result<Secret, TranslationError> {
        let secrets = self
            .secrets
            .lock()
            .map_err(|_| TranslationError::SecretStoreUnavailable)?;
        let secret = secrets
            .get(reference)
            .ok_or(TranslationError::MissingSecret)?;
        Ok(Secret(secret.0.clone()))
    }
}

impl fmt::Debug for SessionSecretStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionSecretStore([REDACTED], memory_only)")
    }
}
