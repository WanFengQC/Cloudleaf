use crate::{
    Language, ProviderIdentity, ProviderOutput, ReadPolicy, TranslationCancellation,
    TranslationDirection, TranslationError, TranslationKind, TranslationRequest,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub directions: Vec<TranslationDirection>,
    pub kinds: Vec<TranslationKind>,
    pub auto_source: bool,
    pub network: bool,
    pub available: bool,
}

pub trait TranslationProvider: Send + Sync + 'static {
    fn identity(&self) -> ProviderIdentity;
    fn capabilities(&self) -> ProviderCapabilities;
    /// 耗时实现仅在后台调用，须合作检查取消和总deadline。
    fn translate(
        &self,
        request: &TranslationRequest,
        cancellation: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationProviderConfig {
    pub enabled: bool,
    pub timeout: Duration,
    pub allow_network: bool,
    pub allow_cache: bool,
    pub config_version: u64,
    pub read_policy: ReadPolicy,
}

impl Default for TranslationProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout: Duration::from_secs(15),
            allow_network: false,
            allow_cache: false,
            config_version: 1,
            read_policy: ReadPolicy::Production,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RegisteredProvider {
    pub provider: Arc<dyn TranslationProvider>,
    pub identity: ProviderIdentity,
    pub capabilities: ProviderCapabilities,
    pub config: TranslationProviderConfig,
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, RegisteredProvider>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        provider: Arc<dyn TranslationProvider>,
        config: TranslationProviderConfig,
    ) -> Result<(), TranslationError> {
        let identity = provider.identity();
        let capabilities = provider.capabilities();
        if self.providers.len() >= 16
            || self.providers.contains_key(&identity.id)
            || identity.id.is_empty()
            || identity.id.len() > 128
            || !identity
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
            || identity.version.trim().is_empty()
            || identity.version.len() > 256
            || config.config_version == 0
            || config.timeout.is_zero()
            || config.timeout > Duration::from_secs(300)
            || capabilities.directions.is_empty()
            || capabilities.directions.len() > 2
            || capabilities.kinds.is_empty()
            || capabilities.kinds.len() > 3
            || capabilities.directions.iter().any(|pair| {
                pair.source == Language::Auto
                    || pair.target == Language::Auto
                    || pair.source == pair.target
            })
        {
            return Err(TranslationError::InvalidConfig);
        }
        self.providers.insert(
            identity.id.clone(),
            RegisteredProvider {
                provider,
                identity,
                capabilities,
                config,
            },
        );
        Ok(())
    }

    pub fn capabilities(&self, id: &str) -> Option<ProviderCapabilities> {
        self.providers
            .get(id)
            .map(|entry| entry.capabilities.clone())
    }

    pub fn identity(&self, id: &str) -> Option<ProviderIdentity> {
        self.providers.get(id).map(|entry| entry.identity.clone())
    }

    pub(crate) fn get(&self, id: &str) -> Result<RegisteredProvider, TranslationError> {
        self.providers
            .get(id)
            .cloned()
            .ok_or(TranslationError::ProviderUnavailable)
    }
}
