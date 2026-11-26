use super::{InputFrame, LocalHost};
use cloudleaf_settings::{
    InputPreferences, InputSettingsStatus, LoadedInputPreferences, RuntimeSettingsReason as Reason,
    RuntimeSettingsStatus as State, RuntimeSettingsWatch,
};
use std::io;
use std::path::PathBuf;

pub(super) struct Runtime {
    watch: RuntimeSettingsWatch,
    active: InputPreferences,
    pending: Option<InputPreferences>,
    observed_revision: Option<u64>,
    generation: u64,
    status: State,
    reason: Option<Reason>,
    accepts_default: bool,
    dictionary_version: Option<String>,
    dirty: bool,
}

impl Runtime {
    pub(super) fn start(directory: PathBuf, loaded: LoadedInputPreferences) -> io::Result<Self> {
        let accepts_default = loaded.status == InputSettingsStatus::DefaultMissing;
        Ok(Self {
            watch: RuntimeSettingsWatch::start(directory)?,
            active: loaded.settings,
            pending: None,
            observed_revision: None,
            generation: 1,
            status: State::Applied,
            reason: None,
            accepts_default,
            dictionary_version: None,
            dirty: true,
        })
    }

    fn reject(&mut self, reason: Reason) {
        self.pending = None;
        self.status = State::Rejected;
        self.reason = Some(reason);
    }

    fn observe(&mut self, loaded: LoadedInputPreferences) {
        self.pending = None;
        self.observed_revision = None;
        if loaded.status != InputSettingsStatus::Current {
            if self.accepts_default && loaded.status == InputSettingsStatus::DefaultMissing {
                self.status = State::Applied;
                self.reason = None;
            } else {
                self.reject(Reason::RecoveredFile);
            }
            return;
        }
        self.accepts_default = false;
        let preferences = loaded.settings;
        if preferences == self.active {
            self.status = State::Applied;
            self.reason = None;
        } else {
            self.observed_revision = Some(preferences.revision);
            if preferences.revision > self.active.revision {
                self.pending = Some(preferences);
                self.status = State::Deferred;
                self.reason = Some(Reason::CompositionOrCommit);
            } else {
                self.reject(Reason::StaleRevision);
            }
        }
    }

    pub(super) fn tick(&mut self, local: &LocalHost) -> Option<InputFrame> {
        if let Some(update) = self.watch.poll() {
            self.dirty = true;
            match update {
                Ok(loaded) => self.observe(loaded),
                Err(_) => {
                    self.observed_revision = None;
                    self.reject(Reason::InvalidFile);
                }
            }
        }
        let attempt = self
            .pending
            .as_ref()
            .map(|preferences| apply(local, preferences));
        let frame = match attempt {
            Some(Ok(Some(frame))) => {
                self.active = self.pending.take().expect("本次待应用配置仍存在");
                self.observed_revision = None;
                self.generation += 1;
                self.status = State::Applied;
                self.reason = None;
                self.dirty = true;
                Some(frame)
            }
            Some(Err(reason)) => {
                self.reject(reason);
                self.dirty = true;
                None
            }
            _ => None,
        };
        let engine = local.host.engine.borrow();
        let dictionary_version = engine.search_snapshot_version();
        if self.dictionary_version.as_deref() != dictionary_version {
            self.dictionary_version = dictionary_version.map(str::to_owned);
            self.dirty = true;
        }
        if self.dirty {
            self.dirty = !self.watch.publish(
                self.generation,
                &self.active,
                self.dictionary_version.as_deref(),
                self.observed_revision,
                self.status,
                self.reason,
            );
        }
        frame
    }
}

pub(super) fn apply(
    local: &LocalHost,
    preferences: &InputPreferences,
) -> Result<Option<InputFrame>, Reason> {
    local.host.check().map_err(|_| Reason::IncompatibleHost)?;
    let mut actual = local.host.engine.borrow_mut();
    let mut projected = local.host.projection.borrow_mut();
    if local.host.quarantined.get()
        || !actual.idle_for_preferences()
        || !projected.idle_for_preferences()
    {
        return Ok(None);
    }
    let before = actual.frame();
    let mut projection = projected.frame();
    projection.revision = before.revision;
    if before != projection
        || actual.search_snapshot_version() != projected.search_snapshot_version()
    {
        return Err(Reason::IncompatibleHost);
    }
    let config = preferences.input_config();
    let scheme = preferences.scheme_config();
    let actual_update = actual
        .prepare_idle_preferences(config, scheme.clone(), preferences.mixed_input)
        .map_err(|_| Reason::IncompatibleHost)?;
    let projected_update = projected
        .prepare_idle_preferences(config, scheme, preferences.mixed_input)
        .map_err(|_| Reason::IncompatibleHost)?;
    actual_update.apply();
    projected_update.apply();
    Ok(Some(actual.frame()))
}
