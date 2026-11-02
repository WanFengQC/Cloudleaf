use crate::manager::{ModelState, VerifiedModel, read_json, verify_catalog};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::marian::{Config, MTModel};
use cloudleaf_translation::{
    Language, OutputKind, ProviderCapabilities, ProviderIdentity, ProviderOutput, QualityStatus,
    TranslationCancellation, TranslationDirection, TranslationError, TranslationKind,
    TranslationProvider, TranslationRequest,
};
use rayon::{ThreadPool, ThreadPoolBuilder};
use sentencepiece_rs::SentencePieceProcessor;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Mutex, TryLockError};
use std::time::Instant;

pub const MAX_INPUT_TOKENS: usize = 256;
pub const MAX_OUTPUT_TOKENS: usize = 256;
pub const COMPUTE_THREADS: usize = 4;

#[derive(Debug, Clone, Serialize)]
pub struct TokenizationObservation {
    pub pieces: Vec<String>,
    pub ids: Vec<u32>,
}

pub struct MarianTokenizer {
    source: SentencePieceProcessor,
    target: SentencePieceProcessor,
    vocab: BTreeMap<String, u32>,
    inverse: Vec<String>,
    prefix: Option<String>,
    unk: u32,
}

impl MarianTokenizer {
    pub fn from_files(
        source: &Path,
        target: &Path,
        vocabulary: &Path,
        target_prefix: Option<&str>,
    ) -> Result<Self, TranslationError> {
        let vocab: BTreeMap<String, u32> = read_json(vocabulary, 16 * 1024 * 1024)?;
        let ids: BTreeSet<_> = vocab.values().copied().collect();
        if vocab.len() != 65001
            || ids.len() != 65001
            || ids.first() != Some(&0)
            || ids.last() != Some(&65000)
            || vocab.get("</s>") != Some(&0)
            || vocab.get("<pad>") != Some(&65000)
            || target_prefix.is_some_and(|prefix| !vocab.contains_key(prefix))
        {
            return Err(TranslationError::InvalidConfig);
        }
        let unk = *vocab.get("<unk>").ok_or(TranslationError::InvalidConfig)?;
        let mut inverse = vec![String::new(); vocab.len()];
        for (piece, id) in &vocab {
            inverse[*id as usize] = piece.clone();
        }
        let source =
            SentencePieceProcessor::open(source).map_err(|_| TranslationError::ModelLoadFailed)?;
        let target =
            SentencePieceProcessor::open(target).map_err(|_| TranslationError::ModelLoadFailed)?;
        Ok(Self {
            source,
            target,
            vocab,
            inverse,
            prefix: target_prefix.map(str::to_owned),
            unk,
        })
    }

    pub fn encode(&self, text: &str) -> Result<TokenizationObservation, TranslationError> {
        let mut pieces = self
            .source
            .encode(text)
            .map_err(|_| TranslationError::InvalidInput)?;
        if let Some(prefix) = &self.prefix {
            pieces.insert(0, prefix.clone());
        }
        let mut ids: Vec<_> = pieces
            .iter()
            .map(|piece| self.vocab.get(piece).copied().unwrap_or(self.unk))
            .collect();
        // Marian的网络词表与SPM内部ID不同，EOS在词表映射后补一次。
        pieces.push("</s>".to_owned());
        ids.push(0);
        Ok(TokenizationObservation { pieces, ids })
    }

    pub fn decode(&self, ids: &[u32]) -> Result<String, TranslationError> {
        let pieces = ids
            .iter()
            .filter(|id| **id != 0 && **id != 65000)
            .map(|id| {
                self.inverse
                    .get(*id as usize)
                    .ok_or(TranslationError::InvalidResponse)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.target
            .decode(&pieces)
            .map_err(|_| TranslationError::InvalidResponse)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InferenceStats {
    pub model_id: String,
    pub revision: String,
    pub runtime: String,
    pub tokenizer: String,
    pub catalog_sha256: String,
    pub load_ms: u64,
    pub inference_ms: u64,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub generated_steps: usize,
    pub output_truncated: bool,
    pub decoding: String,
    pub compute_threads: usize,
    pub resident_models: usize,
}

struct LoadedModel {
    model_id: String,
    revision: String,
    direction: TranslationDirection,
    config: Config,
    tokenizer: MarianTokenizer,
    model: MTModel,
    load_ms: u64,
}

pub struct MarianProvider {
    identity: ProviderIdentity,
    catalog_sha256: String,
    directions: Vec<TranslationDirection>,
    models: Mutex<Vec<LoadedModel>>,
    pool: ThreadPool,
}

impl MarianProvider {
    pub fn load(catalog_path: &Path) -> Result<Self, TranslationError> {
        Self::load_with_progress(catalog_path, |_| {})
    }

    pub fn load_with_progress(
        catalog_path: &Path,
        mut progress: impl FnMut(ModelState),
    ) -> Result<Self, TranslationError> {
        progress(ModelState::Checking);
        let verified = match verify_catalog(catalog_path) {
            Ok(verified) => verified,
            Err(error) => {
                progress(match error {
                    TranslationError::ModelNotFound => ModelState::NotInstalled,
                    TranslationError::InvalidConfig => ModelState::Incompatible,
                    _ => ModelState::Invalid,
                });
                return Err(error);
            }
        };
        progress(ModelState::Ready);
        let pool = ThreadPoolBuilder::new()
            .num_threads(COMPUTE_THREADS)
            .thread_name(|index| format!("cloudleaf-tensor-{index}"))
            .build()
            .map_err(|_| {
                progress(ModelState::Failed);
                TranslationError::ModelLoadFailed
            })?;
        progress(ModelState::Loading);
        let models = pool.install(|| {
            verified
                .models
                .into_iter()
                .map(load_model)
                .collect::<Result<Vec<_>, _>>()
        });
        let models = match models {
            Ok(models) => models,
            Err(error) => {
                progress(ModelState::Failed);
                return Err(error);
            }
        };
        let directions = models.iter().map(|model| model.direction).collect();
        progress(ModelState::Loaded);
        Ok(Self {
            identity: ProviderIdentity {
                id: "offline-model".to_owned(),
                version: verified.sha256.clone(),
            },
            catalog_sha256: verified.sha256,
            directions,
            models: Mutex::new(models),
            pool,
        })
    }

    pub fn translate_observed(
        &self,
        request: &TranslationRequest,
        cancellation: &TranslationCancellation,
    ) -> Result<(ProviderOutput, InferenceStats), TranslationError> {
        request.validate(16 * 1024)?;
        cancellation.check()?;
        let mut models = self.models.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => TranslationError::Busy,
            TryLockError::Poisoned(_) => TranslationError::ProviderUnavailable,
        })?;
        let resident_models = models.len();
        let model = models
            .iter_mut()
            .find(|model| model.direction == request.direction)
            .ok_or(TranslationError::UnsupportedLanguagePair)?;
        self.pool.install(|| {
            model.model.reset_kv_cache();
            let result = infer(
                model,
                request,
                cancellation,
                &self.catalog_sha256,
                resident_models,
            );
            model.model.reset_kv_cache();
            result
        })
    }
}

impl TranslationProvider for MarianProvider {
    fn identity(&self) -> ProviderIdentity {
        self.identity.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            directions: self.directions.clone(),
            kinds: vec![
                TranslationKind::Word,
                TranslationKind::Phrase,
                TranslationKind::Sentence,
            ],
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
        self.translate_observed(request, cancellation)
            .map(|(output, _)| output)
    }
}

fn load_model(verified: VerifiedModel) -> Result<LoadedModel, TranslationError> {
    let started = Instant::now();
    let path_for = |role: &str| {
        verified.root.join(
            &verified
                .manifest
                .files
                .iter()
                .find(|file| file.role == role)
                .expect("已校验角色完整")
                .path,
        )
    };
    let source: HfConfig = read_json(&path_for("config"), 65536)?;
    let config = source.into_config()?;
    let is_en_zh = verified.manifest.direction == "en-zh";
    let tokenizer = MarianTokenizer::from_files(
        &path_for("source_tokenizer"),
        &path_for("target_tokenizer"),
        &path_for("vocabulary"),
        is_en_zh.then_some(">>cmn_Hans<<"),
    )?;
    let weights =
        std::fs::read(path_for("weights")).map_err(|_| TranslationError::ModelLoadFailed)?;
    let vb = VarBuilder::from_buffered_safetensors(weights, DType::F32, &Device::Cpu)
        .map_err(|_| TranslationError::ModelLoadFailed)?;
    let model = MTModel::new(&config, vb).map_err(|_| TranslationError::ModelLoadFailed)?;
    let direction = if is_en_zh {
        TranslationDirection {
            source: Language::English,
            target: Language::Chinese,
        }
    } else {
        TranslationDirection {
            source: Language::Chinese,
            target: Language::English,
        }
    };
    Ok(LoadedModel {
        model_id: verified.manifest.model_id,
        revision: verified.manifest.revision,
        direction,
        config,
        tokenizer,
        model,
        load_ms: started.elapsed().as_millis() as u64,
    })
}

fn infer(
    model: &mut LoadedModel,
    request: &TranslationRequest,
    cancellation: &TranslationCancellation,
    catalog_sha256: &str,
    resident_models: usize,
) -> Result<(ProviderOutput, InferenceStats), TranslationError> {
    let started = Instant::now();
    let encoded = model.tokenizer.encode(&request.text)?;
    if encoded.ids.len() > MAX_INPUT_TOKENS {
        return Err(TranslationError::InputTooLong);
    }
    cancellation.check()?;
    let input = Tensor::new(encoded.ids.as_slice(), &Device::Cpu)
        .and_then(|tensor| tensor.unsqueeze(0))
        .map_err(|_| TranslationError::InternalError)?;
    let encoder = model
        .model
        .encoder()
        .forward(&input, 0)
        .map_err(|_| TranslationError::InternalError)?;
    cancellation.check()?;
    let mut previous = model.config.decoder_start_token_id;
    let mut output = Vec::new();
    let mut terminated = false;
    let mut steps = 0;
    for position in 0..MAX_OUTPUT_TOKENS {
        cancellation.check()?;
        let input = Tensor::new(&[previous], &Device::Cpu)
            .and_then(|tensor| tensor.unsqueeze(0))
            .map_err(|_| TranslationError::InternalError)?;
        let logits = model
            .model
            .decode(&input, &encoder, position)
            .and_then(|tensor| tensor.squeeze(0))
            .and_then(|tensor| tensor.get(0))
            .and_then(|tensor| tensor.to_vec1::<f32>())
            .map_err(|_| TranslationError::InternalError)?;
        cancellation.check()?;
        let next = logits
            .iter()
            .enumerate()
            .filter(|(id, value)| *id != model.config.pad_token_id as usize && value.is_finite())
            .max_by(|(left_id, left), (right_id, right)| {
                left.total_cmp(right).then_with(|| right_id.cmp(left_id))
            })
            .map(|(id, _)| id as u32)
            .ok_or(TranslationError::InvalidResponse)?;
        steps += 1;
        if next == model.config.eos_token_id {
            terminated = true;
            break;
        }
        output.push(next);
        previous = next;
    }
    if !terminated {
        return Err(TranslationError::ResponseTooLarge);
    }
    let translated_text = model.tokenizer.decode(&output)?;
    cancellation.check()?;
    if translated_text.trim().is_empty() {
        return Err(TranslationError::InvalidResponse);
    }
    Ok((
        ProviderOutput {
            translated_text,
            resolved_source: request.direction.source,
            output_kind: OutputKind::Translation,
            quality: QualityStatus::Generated,
            source_ids: vec![format!("{}@{}", model.model_id, model.revision)],
        },
        InferenceStats {
            model_id: model.model_id.clone(),
            revision: model.revision.clone(),
            runtime: "candle-0.9.2-cpu".to_owned(),
            tokenizer: "sentencepiece-rs-0.2.2".to_owned(),
            catalog_sha256: catalog_sha256.to_owned(),
            load_ms: model.load_ms,
            inference_ms: started.elapsed().as_millis() as u64,
            input_tokens: encoded.ids.len(),
            output_tokens: output.len(),
            generated_steps: steps,
            output_truncated: false,
            decoding: "greedy-pad-suppressed".to_owned(),
            compute_threads: COMPUTE_THREADS,
            resident_models,
        },
    ))
}

#[derive(Deserialize)]
struct HfConfig {
    model_type: String,
    vocab_size: usize,
    decoder_vocab_size: Option<usize>,
    max_position_embeddings: usize,
    encoder_layers: usize,
    encoder_ffn_dim: usize,
    encoder_attention_heads: usize,
    decoder_layers: usize,
    decoder_ffn_dim: usize,
    decoder_attention_heads: usize,
    use_cache: bool,
    is_encoder_decoder: bool,
    activation_function: String,
    d_model: usize,
    decoder_start_token_id: u32,
    scale_embedding: bool,
    pad_token_id: u32,
    eos_token_id: u32,
    forced_eos_token_id: u32,
    share_encoder_decoder_embeddings: Option<bool>,
}

impl HfConfig {
    fn into_config(self) -> Result<Config, TranslationError> {
        if self.model_type != "marian"
            || self.vocab_size != 65001
            || self.decoder_vocab_size.is_some_and(|size| size != 65001)
            || self.max_position_embeddings != 512
            || self.encoder_layers != 6
            || self.decoder_layers != 6
            || self.encoder_attention_heads != 8
            || self.decoder_attention_heads != 8
            || self.encoder_ffn_dim != 2048
            || self.decoder_ffn_dim != 2048
            || self.d_model != 512
            || self.decoder_start_token_id != 65000
            || self.pad_token_id != 65000
            || self.eos_token_id != 0
            || self.forced_eos_token_id != 0
            || self.activation_function != "swish"
            || !self.use_cache
            || !self.is_encoder_decoder
            || !self.scale_embedding
            || self.share_encoder_decoder_embeddings == Some(false)
        {
            return Err(TranslationError::InvalidConfig);
        }
        // 老版HF配置省略shared标记；该固定OPUS架构本身共用词嵌入。
        Ok(Config::opus_mt_en_zh())
    }
}
