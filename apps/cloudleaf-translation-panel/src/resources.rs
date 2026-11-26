use cloudleaf_input::SearchSnapshot;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub enum ResourceSlot {
    Base,
    Supplement,
}

impl ResourceSlot {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Base => 0,
            Self::Supplement => 1,
        }
    }
}

pub(crate) struct ResourceRequest {
    pub slot: ResourceSlot,
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Debug)]
pub struct ResourceIdentity {
    pub name: String,
    pub sha256: String,
    pub entries: usize,
    pub bytes: usize,
    pub sources: Vec<String>,
}

pub(crate) fn inspect(request: ResourceRequest) -> Result<ResourceIdentity, &'static str> {
    const MAX_BYTES: u64 = 32 * 1024 * 1024;
    let name = request.path.file_name().ok_or("需要词典文件")?;
    let file = File::open(&request.path).map_err(|_| "无法读取词典文件")?;
    if !file.metadata().is_ok_and(|info| info.is_file()) {
        return Err("需要词典文件");
    }
    let mut text = String::new();
    file.take(MAX_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|_| "词典文件读取失败或编码无效")?;
    if text.len() > MAX_BYTES as usize {
        return Err("词典文件超出检查上限");
    }
    let parsed = SearchSnapshot::from_verified_tsv(&text, &request.sha256)
        .map_err(|_| "词典格式或 SHA-256 不符")?;
    let sources: BTreeSet<_> = parsed
        .entries()
        .iter()
        .map(|entry| format!("{} ({})", entry.source, entry.license_id))
        .collect();
    Ok(ResourceIdentity {
        name: name.to_string_lossy().into_owned(),
        sha256: parsed.version().to_owned(),
        entries: parsed.entries().len(),
        bytes: text.len(),
        sources: sources.into_iter().take(16).collect(),
    })
}
