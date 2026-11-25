use cloudleaf_offline::{MarianProvider, sha256_file};
use cloudleaf_translation::{
    Language, ProviderCapabilities, ProviderIdentity, ProviderOutput, TranslationCancellation,
    TranslationDirection, TranslationError, TranslationKind, TranslationProvider,
    TranslationRequest,
};
use std::{path::PathBuf, sync::Mutex};

/// 由显式的本地目录和预期哈希创建。只有后台工作线程读盘、校验并构造模型。
pub struct LazyMarian {
    catalog: PathBuf,
    identity: ProviderIdentity,
    loaded: Mutex<Option<MarianProvider>>,
}

impl LazyMarian {
    pub fn new(catalog: PathBuf, expected_sha256: String) -> Result<Self, TranslationError> {
        if catalog.as_os_str().is_empty()
            || expected_sha256.len() != 64
            || !expected_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(TranslationError::InvalidConfig);
        }
        Ok(Self {
            catalog,
            identity: ProviderIdentity {
                id: "offline-model".to_owned(),
                version: expected_sha256,
            },
            loaded: Mutex::new(None),
        })
    }
}

impl TranslationProvider for LazyMarian {
    fn identity(&self) -> ProviderIdentity {
        self.identity.clone()
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
            available: true,
        }
    }

    fn translate(
        &self,
        request: &TranslationRequest,
        cancellation: &TranslationCancellation,
    ) -> Result<ProviderOutput, TranslationError> {
        cancellation.check()?;
        let mut loaded = self.loaded.try_lock().map_err(|_| TranslationError::Busy)?;
        if loaded.is_none() {
            if sha256_file(&self.catalog)? != self.identity.version {
                return Err(TranslationError::ModelLoadFailed);
            }
            cancellation.check()?;
            let provider = MarianProvider::load(&self.catalog)?;
            cancellation.check()?;
            if provider.identity() != self.identity {
                return Err(TranslationError::ModelLoadFailed);
            }
            *loaded = Some(provider);
        }
        loaded
            .as_ref()
            .expect("经过校验的本地模型已装载")
            .translate(request, cancellation)
    }
}
