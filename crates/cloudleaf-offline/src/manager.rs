use cloudleaf_translation::TranslationError;
use ring::digest::{Context, SHA256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub const EN_ZH_REVISION: &str = "408d9bc410a388e1d9aef112a2daba955b945255";
pub const ZH_EN_REVISION: &str = "cf109095479db38d6df799875e34039d4938aaa6";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelState {
    NotInstalled,
    Checking,
    Ready,
    Loading,
    Loaded,
    Invalid,
    Incompatible,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCatalog {
    pub schema: u32,
    pub models: Vec<ManifestReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestReference {
    pub manifest: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelManifest {
    pub schema: u32,
    pub model_id: String,
    pub direction: String,
    pub source_url: String,
    pub revision: String,
    pub architecture: String,
    pub runtime: String,
    pub tokenizer: String,
    pub license_id: String,
    pub license_status: String,
    pub files: Vec<ModelFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFile {
    pub role: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInspection {
    pub state: ModelState,
    pub catalog_sha256: Option<String>,
    pub models: Vec<ModelManifest>,
    pub error: Option<String>,
}

pub(crate) struct VerifiedModel {
    pub manifest: ModelManifest,
    pub root: PathBuf,
    pub manifest_relative: PathBuf,
}

pub(crate) struct VerifiedCatalog {
    pub root: PathBuf,
    pub path: PathBuf,
    pub sha256: String,
    pub models: Vec<VerifiedModel>,
}

pub fn inspect_catalog(path: &Path) -> ModelInspection {
    match verify_catalog(path) {
        Ok(verified) => ModelInspection {
            state: ModelState::Ready,
            catalog_sha256: Some(verified.sha256),
            models: verified
                .models
                .into_iter()
                .map(|row| row.manifest)
                .collect(),
            error: None,
        },
        Err(error) => ModelInspection {
            state: match error {
                TranslationError::ModelNotFound => ModelState::NotInstalled,
                TranslationError::InvalidConfig => ModelState::Incompatible,
                _ => ModelState::Invalid,
            },
            catalog_sha256: None,
            models: Vec::new(),
            error: Some(error.to_string()),
        },
    }
}

/// 导入只复制清单中的文件；失败的临时目录不登记为可用版本。
pub fn import_catalog(source: &Path, managed_root: &Path) -> Result<PathBuf, TranslationError> {
    let verified = verify_catalog(source)?;
    fs::create_dir_all(managed_root).map_err(|_| TranslationError::ModelLoadFailed)?;
    let managed_root = managed_root
        .canonicalize()
        .map_err(|_| TranslationError::ModelLoadFailed)?;
    let name = format!("catalog-{}", verified.sha256);
    let destination = managed_root.join(&name);
    if destination.exists() {
        let existing = verify_catalog(&destination.join("catalog.json"))?;
        return if existing.sha256 == verified.sha256 {
            Ok(existing.path)
        } else {
            Err(TranslationError::InvalidConfig)
        };
    }
    let staging = managed_root.join(format!(".{}-{}", name, std::process::id()));
    fs::create_dir(&staging).map_err(|_| TranslationError::Busy)?;
    let mut owned_paths = BTreeSet::new();
    for model in &verified.models {
        copy_file(&verified.root, &staging, &model.manifest_relative)?;
        owned_paths.insert(model.manifest_relative.clone());
        let parent = model
            .manifest_relative
            .parent()
            .ok_or(TranslationError::InvalidConfig)?;
        for file in &model.manifest.files {
            let relative = parent.join(&file.path);
            copy_file(&verified.root, &staging, &relative)?;
            owned_paths.insert(relative);
        }
    }
    fs::copy(&verified.path, staging.join("catalog.json"))
        .map_err(|_| TranslationError::ModelLoadFailed)?;
    let staged = verify_catalog(&staging.join("catalog.json"))?;
    if staged.sha256 != verified.sha256 {
        return Err(TranslationError::ModelLoadFailed);
    }
    let marker = ManagedMarker {
        schema: 1,
        catalog_sha256: verified.sha256,
        files: owned_paths
            .into_iter()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect(),
    };
    fs::write(
        staging.join(".cloudleaf-managed.json"),
        serde_json::to_vec_pretty(&marker).map_err(|_| TranslationError::InternalError)?,
    )
    .map_err(|_| TranslationError::ModelLoadFailed)?;
    fs::rename(staging, &destination).map_err(|_| TranslationError::ModelLoadFailed)?;
    Ok(destination.join("catalog.json"))
}

/// 只卸载本管理器导入且仍完整的版本；遇到额外文件时保留目录供用户处理。
pub fn uninstall_catalog(catalog: &Path, managed_root: &Path) -> Result<(), TranslationError> {
    let verified = verify_catalog(catalog)?;
    let managed_root = managed_root
        .canonicalize()
        .map_err(|_| TranslationError::ModelNotFound)?;
    if verified.root.parent() != Some(managed_root.as_path())
        || verified.root.file_name().and_then(|value| value.to_str())
            != Some(&format!("catalog-{}", verified.sha256))
    {
        return Err(TranslationError::InvalidConfig);
    }
    let marker: ManagedMarker = read_json(&verified.root.join(".cloudleaf-managed.json"), 65536)?;
    if marker.schema != 1 || marker.catalog_sha256 != verified.sha256 {
        return Err(TranslationError::InvalidConfig);
    }
    let mut expected = BTreeSet::from([
        PathBuf::from("catalog.json"),
        PathBuf::from(".cloudleaf-managed.json"),
    ]);
    for model in &verified.models {
        expected.insert(model.manifest_relative.clone());
        let parent = model
            .manifest_relative
            .parent()
            .ok_or(TranslationError::InvalidConfig)?;
        for file in &model.manifest.files {
            expected.insert(parent.join(&file.path));
        }
    }
    let listed: BTreeSet<_> = marker
        .files
        .iter()
        .map(|path| safe_relative(path))
        .collect::<Result<_, _>>()?;
    let marker_expected: BTreeSet<_> = expected
        .iter()
        .filter(|path| {
            **path != PathBuf::from("catalog.json")
                && **path != PathBuf::from(".cloudleaf-managed.json")
        })
        .cloned()
        .collect();
    if listed != marker_expected || actual_files(&verified.root, &verified.root)? != expected {
        return Err(TranslationError::InvalidConfig);
    }
    for relative in expected {
        fs::remove_file(verified.root.join(relative))
            .map_err(|_| TranslationError::ModelLoadFailed)?;
    }
    remove_empty_directories(&verified.root)?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedMarker {
    schema: u32,
    catalog_sha256: String,
    files: Vec<String>,
}

pub(crate) fn verify_catalog(path: &Path) -> Result<VerifiedCatalog, TranslationError> {
    let path = path
        .canonicalize()
        .map_err(|_| TranslationError::ModelNotFound)?;
    let root = path
        .parent()
        .ok_or(TranslationError::InvalidConfig)?
        .to_owned();
    let catalog: ModelCatalog = read_json(&path, 65536)?;
    if catalog.schema != 1 || catalog.models.is_empty() || catalog.models.len() > 2 {
        return Err(TranslationError::InvalidConfig);
    }
    let mut ids = BTreeSet::new();
    let mut models = Vec::new();
    for reference in catalog.models {
        let relative = safe_relative(&reference.manifest)?;
        let manifest_path = contained_file(&root, &relative)?;
        if !valid_sha(&reference.sha256) || sha256_file(&manifest_path)? != reference.sha256 {
            return Err(TranslationError::ModelLoadFailed);
        }
        let manifest: ModelManifest = read_json(&manifest_path, 65536)?;
        validate_manifest(&manifest)?;
        if !ids.insert(manifest.model_id.clone()) {
            return Err(TranslationError::InvalidConfig);
        }
        let model_root = manifest_path
            .parent()
            .ok_or(TranslationError::InvalidConfig)?
            .to_owned();
        for file in &manifest.files {
            let file_path = contained_file(&model_root, &safe_relative(&file.path)?)?;
            if fs::metadata(&file_path)
                .map_err(|_| TranslationError::ModelNotFound)?
                .len()
                != file.bytes
                || sha256_file(&file_path)? != file.sha256
            {
                return Err(TranslationError::ModelLoadFailed);
            }
        }
        models.push(VerifiedModel {
            manifest,
            root: model_root,
            manifest_relative: relative,
        });
    }
    Ok(VerifiedCatalog {
        sha256: sha256_file(&path)?,
        path,
        root,
        models,
    })
}

fn validate_manifest(manifest: &ModelManifest) -> Result<(), TranslationError> {
    let expected = match manifest.model_id.as_str() {
        "opus-mt-en-zh" => ("en-zh", EN_ZH_REVISION),
        "opus-mt-zh-en" => ("zh-en", ZH_EN_REVISION),
        _ => return Err(TranslationError::InvalidConfig),
    };
    if manifest.schema != 1
        || manifest.direction != expected.0
        || manifest.revision != expected.1
        || manifest.source_url
            != format!("https://huggingface.co/Helsinki-NLP/{}", manifest.model_id)
        || manifest.architecture != "marian"
        || manifest.runtime != "candle-0.9.2-cpu"
        || manifest.tokenizer != "sentencepiece-rs-0.2.2"
        || manifest.license_status != "research_only"
        || manifest.license_id != "CC-BY-4.0"
        || manifest.files.len() != 7
    {
        return Err(TranslationError::InvalidConfig);
    }
    let required = BTreeSet::from([
        "weights",
        "config",
        "source_tokenizer",
        "target_tokenizer",
        "vocabulary",
        "license",
        "notice",
    ]);
    let roles: BTreeSet<_> = manifest
        .files
        .iter()
        .map(|file| file.role.as_str())
        .collect();
    let mut paths = BTreeSet::new();
    if roles != required || roles.len() != manifest.files.len() {
        return Err(TranslationError::InvalidConfig);
    }
    for file in &manifest.files {
        let limit = if file.role == "weights" {
            400 * 1024 * 1024
        } else {
            16 * 1024 * 1024
        };
        if file.bytes == 0
            || file.bytes > limit
            || !valid_sha(&file.sha256)
            || !paths.insert(safe_relative(&file.path)?)
        {
            return Err(TranslationError::InvalidConfig);
        }
        if file.role == "license"
            && file.sha256 != "9ba9550ad48438d0836ddab3da480b3b69ffa0aac7b7878b5a0039e7ab429411"
        {
            return Err(TranslationError::InvalidConfig);
        }
        if !matches!(file.role.as_str(), "license" | "notice")
            && pinned_hash(&manifest.direction, &file.role) != Some(file.sha256.as_str())
        {
            return Err(TranslationError::InvalidConfig);
        }
    }
    Ok(())
}

fn pinned_hash(direction: &str, role: &str) -> Option<&'static str> {
    match (direction, role) {
        ("en-zh", "weights") => {
            Some("e5f4423b70cb0f5a40d38d1a2733a77cec2aa23cc0bae647985fe17f8f5fe616")
        }
        ("zh-en", "weights") => {
            Some("a7c75c919608ea1af3bb6d4aca052af43f35e6967a4f133a865bfe2bde13f21e")
        }
        ("en-zh", "config") => {
            Some("bcae8ed74fed77fb51c58462b62397fee6b1a1a34aece79183a0dd02ad329e71")
        }
        ("zh-en", "config") => {
            Some("d84104da12fa90da59acf5f0ed30428e43cc92574ad4b9dfa88f29eeeb2b0eb0")
        }
        ("en-zh", "source_tokenizer") => {
            Some("5775ddc9e3ff2fae91554da56468ad35ff56edaba870fea74447bc7234bfdaa8")
        }
        ("en-zh", "target_tokenizer") => {
            Some("81dc94efa84e4025ef38d25d5d07429fe41e3eb29d44003f1db6fe98487b0052")
        }
        ("zh-en", "source_tokenizer") => {
            Some("e27a3a1b539f4959ec72ea60e453f49156289f95d4e6000b29332efc45616203")
        }
        ("zh-en", "target_tokenizer") => {
            Some("6a881f4717cd7265f53fea54fd3dc689c767c05338fac7a4590f3088cb2d7855")
        }
        ("en-zh", "vocabulary") => {
            Some("f1832ca38f7aa158b9a944ff583e4901e6b37d9d4ad9241f03838e636e9cfb03")
        }
        ("zh-en", "vocabulary") => {
            Some("c0f79ee4c413ccb24cd808a0c727eec85b2130f23c82802cc1caeb07b5f0d458")
        }
        _ => None,
    }
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    max_bytes: u64,
) -> Result<T, TranslationError> {
    let file = File::open(path).map_err(|_| TranslationError::ModelNotFound)?;
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| TranslationError::ModelLoadFailed)?;
    if bytes.len() as u64 > max_bytes {
        return Err(TranslationError::InvalidConfig);
    }
    serde_json::from_slice(&bytes).map_err(|_| TranslationError::InvalidConfig)
}

pub fn sha256_file(path: &Path) -> Result<String, TranslationError> {
    let mut file = File::open(path).map_err(|_| TranslationError::ModelNotFound)?;
    let mut context = Context::new(&SHA256);
    let mut buffer = [0u8; 65536];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| TranslationError::ModelLoadFailed)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }
    Ok(context
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn valid_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn safe_relative(value: &str) -> Result<PathBuf, TranslationError> {
    if value.is_empty() || value.len() > 512 || value.contains(['\\', ':', '\0']) {
        return Err(TranslationError::InvalidConfig);
    }
    let path = PathBuf::from(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
        || path.components().any(|component| {
            component.as_os_str().to_str().is_some_and(|name| {
                name.eq_ignore_ascii_case("catalog.json")
                    || name.eq_ignore_ascii_case(".cloudleaf-managed.json")
                    || name.ends_with(['.', ' '])
            })
        })
    {
        return Err(TranslationError::InvalidConfig);
    }
    Ok(path)
}

fn contained_file(root: &Path, relative: &Path) -> Result<PathBuf, TranslationError> {
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|_| TranslationError::ModelNotFound)?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(TranslationError::InvalidConfig);
    }
    Ok(path)
}

fn copy_file(source: &Path, target: &Path, relative: &Path) -> Result<(), TranslationError> {
    let destination = target.join(relative);
    fs::create_dir_all(
        destination
            .parent()
            .ok_or(TranslationError::InvalidConfig)?,
    )
    .map_err(|_| TranslationError::ModelLoadFailed)?;
    fs::copy(source.join(relative), destination).map_err(|_| TranslationError::ModelLoadFailed)?;
    Ok(())
}

fn actual_files(root: &Path, directory: &Path) -> Result<BTreeSet<PathBuf>, TranslationError> {
    let mut files = BTreeSet::new();
    for entry in fs::read_dir(directory).map_err(|_| TranslationError::ModelLoadFailed)? {
        let entry = entry.map_err(|_| TranslationError::ModelLoadFailed)?;
        let kind = entry
            .file_type()
            .map_err(|_| TranslationError::ModelLoadFailed)?;
        if kind.is_symlink() {
            return Err(TranslationError::InvalidConfig);
        }
        if kind.is_dir() {
            files.extend(actual_files(root, &entry.path())?);
        } else {
            files.insert(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| TranslationError::InvalidConfig)?
                    .to_owned(),
            );
        }
    }
    Ok(files)
}

fn remove_empty_directories(directory: &Path) -> Result<(), TranslationError> {
    for entry in fs::read_dir(directory).map_err(|_| TranslationError::ModelLoadFailed)? {
        let path = entry.map_err(|_| TranslationError::ModelLoadFailed)?.path();
        if path.is_dir() {
            remove_empty_directories(&path)?;
        }
    }
    fs::remove_dir(directory).map_err(|_| TranslationError::ModelLoadFailed)
}
