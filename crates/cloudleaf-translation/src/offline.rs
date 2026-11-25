use crate::{
    Language, ProviderCapabilities, ProviderIdentity, ProviderOutput, TranslationCancellation,
    TranslationDirection, TranslationError, TranslationKind, TranslationProvider,
    TranslationRequest,
};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineModelDescriptor {
    pub identity: ProviderIdentity,
    pub model_path: PathBuf,
    pub model_version: String,
    pub license_id: Option<String>,
}

pub struct OfflineModelProvider {
    descriptor: OfflineModelDescriptor,
}

impl OfflineModelProvider {
    pub fn unavailable(descriptor: OfflineModelDescriptor) -> Result<Self, TranslationError> {
        if descriptor.identity.id.trim().is_empty() || descriptor.model_version.trim().is_empty() {
            return Err(TranslationError::InvalidConfig);
        }
        Ok(Self { descriptor })
    }

    pub fn descriptor(&self) -> &OfflineModelDescriptor {
        &self.descriptor
    }
}

impl TranslationProvider for OfflineModelProvider {
    fn identity(&self) -> ProviderIdentity {
        self.descriptor.identity.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            directions: vec![
                TranslationDirection {
                    source: Language::Chinese,
                    target: Language::English,
                },
                TranslationDirection {
                    source: Language::English,
                    target: Language::Chinese,
                },
            ],
            kinds: vec![TranslationKind::Sentence],
            auto_source: false,
            network: false,
            available: false,
        }
    }

    fn translate(
        &self,
        _request: &TranslationRequest,
        cancellation: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError> {
        cancellation.check()?;
        // 路径元数据不是已加载的推理后端，不扫描磁盘或制造句子译文。
        Err(TranslationError::ModelNotFound)
    }
}
