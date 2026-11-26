use cloudleaf_settings::{
    InputPreferences, InputSettingsError, InputSettingsStatus, LoadedInputPreferences,
};
use cloudleaf_user::{ErrorKind, Store, UserEntry};
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct InputSettingsView {
    pub settings: InputPreferences,
    pub working: bool,
    pub serial: u64,
    pub notice: String,
}

impl InputSettingsView {
    pub(crate) fn complete(
        &mut self,
        result: Result<LoadedInputPreferences, InputSettingsError>,
        saved: bool,
    ) {
        match result {
            Ok(loaded) => {
                self.settings = loaded.settings;
                self.notice = match loaded.status {
                    InputSettingsStatus::Current if saved => "已保存，运行状态见宿主回执",
                    InputSettingsStatus::Current => "已读取设置",
                    InputSettingsStatus::Backup => "已读取有效备份，原文件保留",
                    InputSettingsStatus::DefaultMissing => "首次启动，使用默认设置",
                    InputSettingsStatus::DefaultRecovered => "设置损坏，使用默认值，原文件保留",
                }
                .into();
                self.serial += 1;
            }
            Err(error) => self.notice = error.to_string(),
        }
        self.working = false;
    }
}

pub enum UserAction {
    Inspect,
    Add {
        text: String,
        pinyin: String,
    },
    Update {
        id: String,
        text: String,
        pinyin: String,
    },
    Remove(String),
    Import(PathBuf),
    Export(PathBuf),
    Recover(u64),
    ChangeLearning(bool),
    ClearLearning,
}

#[derive(Debug, Default)]
pub struct UserDictionaryView {
    pub generation: Option<u64>,
    pub learning: bool,
    pub entries: Vec<UserEntry>,
    pub backups: Vec<u64>,
    pub recovery_required: bool,
}

#[derive(Debug, Default)]
pub struct UserSettingsView {
    pub dictionary: UserDictionaryView,
    pub directory: Option<PathBuf>,
    pub(crate) pending_directory: Option<PathBuf>,
    pub working: bool,
    pub serial: u64,
    pub notice: String,
    pub needs_refresh: bool,
}

impl UserSettingsView {
    pub(crate) fn complete(&mut self, result: Result<UserDictionaryView, cloudleaf_user::Error>) {
        match result {
            Ok(view) => {
                self.notice = if view.recovery_required {
                    "当前版本损坏，请核对可恢复版本后明确恢复"
                } else {
                    "用户词库操作完成"
                }
                .into();
                self.dictionary = view;
                self.directory = self.pending_directory.take();
                self.needs_refresh = false;
                self.serial += 1;
            }
            Err(error) => {
                self.needs_refresh |=
                    matches!(error.kind, ErrorKind::Unknown | ErrorKind::Conflict);
                self.pending_directory = None;
                self.notice = error.to_string();
            }
        }
        self.working = false;
    }
}

pub(crate) fn user_action(
    directory: &Path,
    expected_generation: Option<u64>,
    action: UserAction,
) -> cloudleaf_user::Result<UserDictionaryView> {
    let mut store = Store::open(directory.to_owned())?;
    if let UserAction::Recover(generation) = action {
        let dictionary = store.recover(generation)?;
        return user_view(&store, &dictionary);
    }
    let mut dictionary = match store.load() {
        Ok(dictionary) => dictionary,
        Err(error)
            if error.kind == ErrorKind::RecoveryRequired
                && matches!(action, UserAction::Inspect) =>
        {
            return Ok(UserDictionaryView {
                backups: store.recovery_candidates()?,
                recovery_required: true,
                ..UserDictionaryView::default()
            });
        }
        Err(error) => return Err(error),
    };
    let generation = dictionary.generation();
    if !matches!(action, UserAction::Inspect) && expected_generation != Some(generation) {
        return Err(cloudleaf_user::Error {
            kind: ErrorKind::Conflict,
            os_code: None,
        });
    }
    match action {
        UserAction::Inspect => return user_view(&store, &dictionary),
        UserAction::Add { text, pinyin } => {
            dictionary.add(&text, &pinyin)?;
        }
        UserAction::Update { id, text, pinyin } => dictionary.update(&id, &text, &pinyin)?,
        UserAction::Remove(id) => dictionary.remove(&id)?,
        UserAction::Import(path) => dictionary = store.import(&path, &dictionary)?,
        UserAction::Export(path) => {
            store.export(&dictionary, &path)?;
            return user_view(&store, &dictionary);
        }
        UserAction::ChangeLearning(enabled) => dictionary.set_learning_enabled(enabled)?,
        UserAction::ClearLearning => dictionary.clear_learning()?,
        UserAction::Recover(_) => unreachable!("上方已处理恢复流程"),
    }
    let saved = store.save(&dictionary, generation)?;
    user_view(&store, &saved)
}

fn user_view(
    store: &Store,
    dictionary: &cloudleaf_user::UserDictionary,
) -> cloudleaf_user::Result<UserDictionaryView> {
    Ok(UserDictionaryView {
        generation: Some(dictionary.generation()),
        learning: dictionary.learning_enabled(),
        entries: dictionary.entries().values().cloned().collect(),
        backups: store.recovery_candidates()?,
        recovery_required: false,
    })
}
