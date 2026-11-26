use cloudleaf_translation::{ReadPolicy, SemanticDictionary, TranslationError};
use std::fmt::Write;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefinitionDirection {
    ChineseHeadword,
    EnglishGloss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefinitionTask {
    pub id: u64,
    pub input_revision: u64,
}

#[derive(Debug, Default)]
pub struct DefinitionView {
    pub task: Option<DefinitionTask>,
    pub pending: bool,
    pub notice: String,
    pub contents: String,
}

pub(crate) struct DefinitionRequest {
    pub task: DefinitionTask,
    pub path: PathBuf,
    pub sha256: String,
    pub text: String,
    pub direction: DefinitionDirection,
}

pub(crate) struct DefinitionLookup {
    pub total: usize,
    pub text: String,
}

#[derive(Default)]
pub(crate) struct DictionaryCache {
    current: Option<(PathBuf, String, SemanticDictionary)>,
}

impl DictionaryCache {
    pub fn lookup(
        &mut self,
        request: DefinitionRequest,
    ) -> Result<DefinitionLookup, TranslationError> {
        if self
            .current
            .as_ref()
            .is_none_or(|(path, sha, _)| path != &request.path || sha != &request.sha256)
        {
            let dictionary =
                SemanticDictionary::load(&request.path, &request.sha256, ReadPolicy::Research)?;
            self.current = Some((request.path, request.sha256, dictionary));
        }
        let (_, _, dictionary) = self.current.as_ref().unwrap();
        let hits = match request.direction {
            DefinitionDirection::ChineseHeadword => dictionary.headword("zh", &request.text),
            DefinitionDirection::EnglishGloss => dictionary.english_gloss_exact(&request.text),
        };
        let mut text = format!(
            "ResearchOnly · {} 条义项\n语义索引 SHA-256：{}\n",
            hits.total,
            dictionary.lookup_index_sha256()
        );
        let mut shown = 0;
        for hit in hits.hits {
            let mut item = String::new();
            let _ = write!(
                item,
                "\n{}\n词条：{} · {}\n义项：{}\n词性：{}\n{}\n",
                hit.entry.headword,
                hit.entry.namespace,
                hit.entry.entry_id,
                hit.sense.id,
                hit.sense
                    .part_of_speech
                    .as_ref()
                    .map_or("来源未提供", |pos| pos.tag.as_str()),
                hit.sense.definition
            );
            for source in &hit.sense.assertions {
                let _ = writeln!(
                    item,
                    "来源：{} · {} · {}",
                    source.source_id, source.unit_id, source.pointer
                );
            }
            if text.len() + item.len() > 112 * 1024 {
                break;
            }
            shown += 1;
            text.push_str(&item);
        }
        if shown < hits.total {
            let _ = writeln!(text, "\n已显示 {shown} / {} 条义项。", hits.total);
        }
        Ok(DefinitionLookup {
            total: hits.total,
            text,
        })
    }
}
