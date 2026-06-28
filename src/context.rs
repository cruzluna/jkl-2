use blake3;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use thiserror::Error;

#[derive(
    Clone, Debug, Deserialize, Serialize, PartialEq, Eq, strum::Display, strum::EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum AgentStatus {
    // When the agent is waiting for new work or review.
    #[serde(alias = "waiting", alias = "done")]
    #[strum(to_string = "idle", serialize = "waiting", serialize = "done")]
    Idle,
    // When the agent is actively working.
    Working,
    // When the agent needs human input or permission to proceed.
    Blocked,
    // When the agent status is explicitly unknown.
    #[serde(alias = "none")]
    #[strum(to_string = "unknown", serialize = "none")]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct PaneContext {
    pub pane_id: Option<String>,
    // Denormalized convenience fields for display/filtering on pane records.
    // `window_id` is the canonical link; `window_name` is a cached label.
    pub window_id: Option<String>,
    pub window_name: Option<String>,
    pub pane_name: Option<String>,
    pub pane_status: Option<AgentStatus>,
    pub pane_context: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct WindowContext {
    pub window_id: Option<String>,
    pub window_name: Option<String>,
    pub window_status: Option<AgentStatus>,
    pub window_context: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct SessionContext {
    pub session_name: Option<String>,
    pub session_id: Option<String>,
    // TODO: session_status might be deprecated once derived session statuses are stable.
    pub session_status: Option<AgentStatus>,
    pub session_context: Option<String>,
    #[serde(default)]
    pub windows: WindowContextJson,
    #[serde(default)]
    pub panes: PaneContextJson,
}

pub type SessionKey = String;
pub type PaneId = String;

pub type ContextJson = HashMap<SessionKey, SessionContext>;
pub type WindowId = String;
pub type WindowContextJson = HashMap<WindowId, WindowContext>;
pub type PaneContextJson = HashMap<PaneId, PaneContext>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub removed_sessions: usize,
    pub removed_panes: usize,
    pub renamed_sessions: usize,
}

// TODO: these are too specific, should generalize
#[derive(Error, Debug)]
pub enum ContextError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Environment variable not found: {0}")]
    EnvVar(#[from] std::env::VarError),
    #[error("Failed to save contexts: {0}")]
    FailedToSave(String),
    #[error("Invalid status: {0}")]
    InvalidStatus(String),
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
}

pub fn session_key(session_name: &str) -> String {
    blake3::hash(session_name.as_bytes()).to_hex().to_string()
}

pub fn effective_session_status<I>(
    override_status: Option<AgentStatus>,
    pane_statuses: I,
) -> Option<AgentStatus>
where
    I: IntoIterator<Item = Option<AgentStatus>>,
{
    if override_status.is_some() {
        return override_status;
    }

    let mut blocked = false;
    let mut working = false;
    let mut idle = false;
    let mut unknown = false;

    for status in pane_statuses {
        let Some(status) = status else {
            continue;
        };
        match status {
            AgentStatus::Blocked => blocked = true,
            AgentStatus::Working => working = true,
            AgentStatus::Idle => idle = true,
            AgentStatus::Unknown => unknown = true,
        }
    }

    if blocked {
        return Some(AgentStatus::Blocked);
    }
    if working {
        return Some(AgentStatus::Working);
    }
    if idle {
        return Some(AgentStatus::Idle);
    }
    if unknown {
        return Some(AgentStatus::Unknown);
    }

    None
}

/// Try to load the context file, create it if it doesn't exist
pub fn load_contexts() -> Result<ContextJson, ContextError> {
    let path = context_path()?;
    debug!("loading contexts path={}", path.display());
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            info!(
                "context file missing, creating new file at {}",
                path.display()
            );
            // Write the empty context to the file
            fs::write(&path, "{}")?;
            return Ok(ContextJson::new());
        }
        Err(error) => return Err(ContextError::Io(error)),
    };
    debug!("loaded context file contents length={}", contents.len());
    let contexts = serde_json::from_str(&contents)?;
    let normalized = normalize_context_keys(contexts);
    info!("loaded {} session contexts", normalized.len());
    debug!("contexts: {:?}", normalized);
    Ok(normalized)
}

pub fn upsert_session(
    session_name: String,
    session_id: Option<String>,
    status: Option<AgentStatus>,
    context: Option<String>,
) -> Result<String, ContextError> {
    info!(
        "upsert session name={} session_id={:?} status={:?} context_present={}",
        session_name,
        session_id,
        status,
        context.is_some()
    );
    let mut session_contexts = load_contexts()?;
    let key = session_key(&session_name);
    let entry = session_contexts.entry(key.clone()).or_default();
    entry.session_name = Some(session_name);
    if let Some(session_id) = session_id {
        entry.session_id = Some(session_id);
    }
    if status.is_some() {
        entry.session_status = status;
    }
    if context.is_some() {
        entry.session_context = context;
    }
    save_contexts(&session_contexts)?;
    Ok(key)
}

pub fn upsert_pane(
    session_name: &str,
    pane_id: &str,
    window_id: Option<String>,
    window_name: Option<String>,
    pane_name: Option<String>,
    pane_status: Option<AgentStatus>,
    pane_context: Option<String>,
) -> Result<(), ContextError> {
    info!(
        "upsert pane session_name={} pane_id={} status={:?} context_present={}",
        session_name,
        pane_id,
        pane_status,
        pane_context.is_some()
    );
    let mut contexts = load_contexts()?;
    let entry = contexts.entry(session_key(session_name)).or_default();
    entry.session_name = Some(session_name.to_string());
    let pane = entry.panes.entry(pane_id.to_string()).or_default();
    pane.pane_id = Some(pane_id.to_string());

    if let Some(window_id) = window_id {
        pane.window_id = Some(window_id.clone());
        if let Some(window_name) = window_name.clone() {
            pane.window_name = Some(window_name.clone());
            let window = entry.windows.entry(window_id.clone()).or_default();
            if window.window_id.is_none() {
                window.window_id = Some(window_id);
            }
            window.window_name = Some(window_name);
        } else {
            entry
                .windows
                .entry(window_id.clone())
                .or_insert_with(|| WindowContext {
                    window_id: Some(window_id),
                    ..WindowContext::default()
                });
        }
    }

    if let Some(pane_name) = pane_name {
        pane.pane_name = Some(pane_name);
    }

    if let Some(status) = pane_status {
        pane.pane_status = Some(status);
    }
    if let Some(context) = pane_context {
        pane.pane_context = Some(context);
    }
    save_contexts(&contexts)?;
    Ok(())
}

pub fn rename_session(session_id: &str, session_name: &str) -> Result<(), ContextError> {
    info!(
        "rename session session_id={} new_name={}",
        session_id, session_name
    );
    let mut contexts = load_contexts()?;
    let mut extracted = None;
    let mut old_key = None;
    for (key, context) in &contexts {
        if context.session_id.as_deref() == Some(session_id) {
            old_key = Some(key.clone());
            extracted = Some(context.clone());
            break;
        }
    }
    if let Some(old_key) = old_key {
        contexts.remove(&old_key);
    }
    let mut entry = extracted.unwrap_or_default();
    entry.session_name = Some(session_name.to_string());
    entry.session_id = Some(session_id.to_string());
    let new_key = session_key(session_name);
    let target = contexts.entry(new_key).or_default();
    merge_context(target, entry);
    save_contexts(&contexts)?;
    Ok(())
}

pub fn sync_with_tmux(
    live_sessions: &[crate::tmux::TmuxSession],
    live_panes: &[crate::tmux::TmuxPane],
) -> Result<SyncSummary, ContextError> {
    info!(
        "syncing contexts against tmux: {} sessions, {} panes",
        live_sessions.len(),
        live_panes.len()
    );
    let contexts = load_contexts()?;

    let live_by_id: HashMap<&str, &crate::tmux::TmuxSession> = live_sessions
        .iter()
        .map(|session| (session.id.as_str(), session))
        .collect();
    let live_by_name: HashMap<&str, &crate::tmux::TmuxSession> = live_sessions
        .iter()
        .map(|session| (session.name.as_str(), session))
        .collect();

    let mut live_pane_ids: HashMap<String, HashSet<String>> = HashMap::new();
    let mut live_window_ids: HashMap<String, HashSet<String>> = HashMap::new();
    let mut live_window_names: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut pane_window_by_session: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut pane_window_name_by_session: HashMap<String, HashMap<String, String>> = HashMap::new();
    for pane in live_panes {
        live_pane_ids
            .entry(pane.session_name.clone())
            .or_default()
            .insert(pane.pane_id.clone());
        live_window_ids
            .entry(pane.session_name.clone())
            .or_default()
            .insert(pane.window_id.clone());
        live_window_names
            .entry(pane.session_name.clone())
            .or_default()
            .insert(pane.window_id.clone(), pane.window_name.clone());
        pane_window_by_session
            .entry(pane.session_name.clone())
            .or_default()
            .insert(pane.pane_id.clone(), pane.window_id.clone());
        pane_window_name_by_session
            .entry(pane.session_name.clone())
            .or_default()
            .insert(pane.pane_id.clone(), pane.window_name.clone());
    }

    let mut summary = SyncSummary::default();
    let mut synced = HashMap::new();
    for mut context in contexts.into_values() {
        let matched_session = context
            .session_id
            .as_deref()
            .and_then(|id| live_by_id.get(id).copied())
            .or_else(|| {
                context
                    .session_name
                    .as_deref()
                    .and_then(|name| live_by_name.get(name).copied())
            });

        let Some(live_session) = matched_session else {
            summary.removed_sessions += 1;
            summary.removed_panes += context.panes.len();
            continue;
        };

        if context
            .session_name
            .as_deref()
            .is_some_and(|name| name != live_session.name)
        {
            summary.renamed_sessions += 1;
        }

        context.session_name = Some(live_session.name.clone());
        context.session_id = Some(live_session.id.clone());

        let before_panes = context.panes.len();
        let before_windows = context.windows.len();
        if let Some(live_ids) = live_pane_ids.get(&live_session.name) {
            context
                .panes
                .retain(|pane_id, _| live_ids.contains(pane_id));
            if let Some(pane_to_window) = pane_window_by_session.get(&live_session.name) {
                for (pane_id, pane) in &mut context.panes {
                    if let Some(window_id) = pane_to_window.get(pane_id) {
                        pane.window_id = Some(window_id.clone());
                    }
                    if let Some(window_name) = pane_window_name_by_session
                        .get(&live_session.name)
                        .and_then(|map| map.get(pane_id))
                    {
                        pane.window_name = Some(window_name.clone());
                    }
                }
            }
        } else {
            context.panes.clear();
        }

        if let Some(live_windows) = live_window_ids.get(&live_session.name) {
            context
                .windows
                .retain(|window_id, _| live_windows.contains(window_id));
            if let Some(window_names) = live_window_names.get(&live_session.name) {
                for (window_id, window_name) in window_names {
                    let window = context.windows.entry(window_id.clone()).or_default();
                    window.window_id = Some(window_id.clone());
                    window.window_name = Some(window_name.clone());
                }
            }
        } else {
            context.windows.clear();
        }
        summary.removed_panes += before_panes.saturating_sub(context.panes.len());
        let _ = before_windows;

        let key = session_key(&live_session.name);
        let target = synced.entry(key).or_default();
        merge_context(target, context);
    }

    info!(
        "sync complete: removed_sessions={} removed_panes={} renamed_sessions={}",
        summary.removed_sessions, summary.removed_panes, summary.renamed_sessions
    );
    save_contexts(&synced)?;
    Ok(summary)
}

pub fn prune_panes(live_panes: &HashMap<String, HashSet<String>>) -> Result<(), Box<dyn Error>> {
    debug!("pruning panes for {} sessions", live_panes.len());
    let mut contexts = load_contexts()?;
    let before_count: usize = contexts.values().map(|ctx| ctx.panes.len()).sum();
    for context in contexts.values_mut() {
        let Some(session_name) = context.session_name.as_ref() else {
            continue;
        };
        let Some(live_ids) = live_panes.get(session_name) else {
            continue;
        };
        context
            .panes
            .retain(|pane_id, _| live_ids.contains(pane_id));

        let live_windows: HashSet<String> = context
            .panes
            .values()
            .filter_map(|pane| pane.window_id.clone())
            .collect();
        context
            .windows
            .retain(|window_id, _| live_windows.contains(window_id));
    }
    let after_count: usize = contexts.values().map(|ctx| ctx.panes.len()).sum();
    if after_count < before_count {
        info!(
            "pruned {} stale panes",
            before_count.saturating_sub(after_count)
        );
    }
    save_contexts(&contexts)?;
    Ok(())
}

fn normalize_context_keys(contexts: ContextJson) -> ContextJson {
    debug!("normalizing {} context keys", contexts.len());
    let mut normalized = HashMap::new();
    for (key, context) in contexts {
        let session_name = context.session_name.as_deref();
        let normalized_key = session_name.map(session_key).unwrap_or_else(|| key.clone());
        debug!(
            "normalizing: old_key={} session_name={:?} new_key={}",
            key, session_name, normalized_key
        );
        let entry = normalized.entry(normalized_key).or_default();
        merge_context(entry, context);
    }
    debug!("normalized to {} contexts", normalized.len());
    normalized
}

fn merge_context(target: &mut SessionContext, source: SessionContext) {
    if target.session_name.is_none() {
        target.session_name = source.session_name;
    }
    if target.session_id.is_none() {
        target.session_id = source.session_id;
    }
    if target.session_status.is_none() {
        target.session_status = source.session_status;
    }
    if target.session_context.is_none() {
        target.session_context = source.session_context;
    }
    for (window_id, window) in source.windows {
        let WindowContext {
            window_id: source_window_id,
            window_name: source_window_name,
            window_status: source_window_status,
            window_context: source_window_context,
        } = window;
        let entry = target.windows.entry(window_id).or_default();
        if entry.window_id.is_none() {
            entry.window_id = source_window_id;
        }
        if entry.window_name.is_none() {
            entry.window_name = source_window_name;
        }
        if entry.window_status.is_none() {
            entry.window_status = source_window_status;
        }
        if entry.window_context.is_none() {
            entry.window_context = source_window_context;
        }
    }
    for (pane_id, pane) in source.panes {
        let PaneContext {
            pane_id: source_pane_id,
            window_id: source_window_id,
            window_name: source_window_name,
            pane_name: source_pane_name,
            pane_status: source_pane_status,
            pane_context: source_pane_context,
        } = pane;
        let entry = target.panes.entry(pane_id).or_default();
        if entry.pane_id.is_none() {
            entry.pane_id = source_pane_id;
        }
        if entry.window_id.is_none() {
            entry.window_id = source_window_id;
        }
        if entry.window_name.is_none() {
            entry.window_name = source_window_name;
        }
        if entry.pane_name.is_none() {
            entry.pane_name = source_pane_name;
        }
        if entry.pane_status.is_none() {
            entry.pane_status = source_pane_status;
        }
        if entry.pane_context.is_none() {
            entry.pane_context = source_pane_context;
        }
    }
}

fn save_contexts(contexts: &ContextJson) -> Result<(), ContextError> {
    let path = context_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    debug!(
        "saving {} session contexts to {}",
        contexts.len(),
        path.display()
    );
    let contents = serde_json::to_string_pretty(contexts)?;
    let temp_path = path.with_extension(format!(
        "json-{}.tmp",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| ContextError::FailedToSave(e.to_string()))?
            .as_millis()
    ));
    fs::write(&temp_path, contents)?;
    fs::rename(&temp_path, &path)?;
    Ok(())
}

/// Get the path to the context file
/// Should return $HOME/.config/jkl/session_context.json
fn context_path() -> Result<PathBuf, ContextError> {
    let home = std::env::var("HOME")?;
    let base_dir = PathBuf::from(home).join(".config");
    let path = base_dir.join("jkl").join("session_context.json");
    debug!("resolved context path={}", path.display());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;
    use crate::tmux::{TmuxPane, TmuxSession};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn session_key_is_stable() {
        let first = session_key("alpha");
        let second = session_key("alpha");
        assert_eq!(first, second);
        assert_ne!(first, session_key("beta"));
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn effective_session_status_prioritizes_blocked_then_working_then_idle() {
        let status = effective_session_status(
            None,
            vec![Some(AgentStatus::Unknown), Some(AgentStatus::Idle)],
        );
        assert_eq!(status, Some(AgentStatus::Idle));

        let status = effective_session_status(
            None,
            vec![
                Some(AgentStatus::Unknown),
                Some(AgentStatus::Idle),
                Some(AgentStatus::Working),
            ],
        );
        assert_eq!(status, Some(AgentStatus::Working));

        let status = effective_session_status(
            None,
            vec![
                Some(AgentStatus::Working),
                Some(AgentStatus::Blocked),
                Some(AgentStatus::Idle),
            ],
        );
        assert_eq!(status, Some(AgentStatus::Blocked));
    }

    #[test]
    fn effective_session_status_uses_unknown_when_only_unknown_is_reported() {
        let status = effective_session_status(None, vec![Some(AgentStatus::Unknown), None]);
        assert_eq!(status, Some(AgentStatus::Unknown));
    }

    #[test]
    fn agent_status_reads_legacy_values_as_new_statuses() {
        assert_eq!("waiting".parse::<AgentStatus>(), Ok(AgentStatus::Idle));
        assert_eq!("done".parse::<AgentStatus>(), Ok(AgentStatus::Idle));
        assert_eq!("none".parse::<AgentStatus>(), Ok(AgentStatus::Unknown));

        let idle: AgentStatus = serde_json::from_str("\"waiting\"").expect("legacy waiting");
        let done: AgentStatus = serde_json::from_str("\"done\"").expect("legacy done");
        let unknown: AgentStatus = serde_json::from_str("\"none\"").expect("legacy none");

        assert_eq!(idle, AgentStatus::Idle);
        assert_eq!(done, AgentStatus::Idle);
        assert_eq!(unknown, AgentStatus::Unknown);
        assert_eq!(
            serde_json::to_string(&AgentStatus::Idle).expect("serialize idle"),
            "\"idle\""
        );
    }

    #[test]
    fn load_contexts_creates_missing_file() {
        let mut env = EnvGuard::new("context-load");
        env.set_temp_home();
        let path = context_path().expect("context path");
        assert!(!path.exists());

        let contexts = load_contexts().expect("load contexts");

        assert!(contexts.is_empty());
        assert!(path.exists());
    }

    #[test]
    fn upsert_session_persists_fields() {
        let mut env = EnvGuard::new("context-upsert-session");
        env.set_temp_home();

        let key = upsert_session(
            "Alpha".to_string(),
            Some("id1".to_string()),
            Some(AgentStatus::Working),
            Some("hello".to_string()),
        )
        .expect("upsert session");

        let contexts = load_contexts().expect("load contexts");
        let entry = contexts.get(&key).expect("session entry");
        assert_eq!(entry.session_name.as_deref(), Some("Alpha"));
        assert_eq!(entry.session_id.as_deref(), Some("id1"));
        assert_eq!(entry.session_status, Some(AgentStatus::Working));
        assert_eq!(entry.session_context.as_deref(), Some("hello"));
    }

    #[test]
    fn upsert_pane_adds_pane() {
        let mut env = EnvGuard::new("context-upsert-pane");
        env.set_temp_home();

        upsert_pane(
            "Alpha",
            "%1",
            None,
            None,
            Some("pane".to_string()),
            Some(AgentStatus::Idle),
            Some("ctx".to_string()),
        )
        .expect("upsert pane");

        let contexts = load_contexts().expect("load contexts");
        let session = contexts.get(&session_key("Alpha")).expect("session entry");
        let pane = session.panes.get("%1").expect("pane entry");
        assert_eq!(pane.pane_name.as_deref(), Some("pane"));
        assert_eq!(pane.pane_status, Some(AgentStatus::Idle));
        assert_eq!(pane.pane_context.as_deref(), Some("ctx"));
    }

    #[test]
    fn upsert_pane_ignores_window_name_without_window_id() {
        let mut env = EnvGuard::new("context-upsert-pane-window-name-only");
        env.set_temp_home();

        upsert_pane(
            "Alpha",
            "%1",
            None,
            Some("editor".to_string()),
            None,
            None,
            None,
        )
        .expect("upsert pane");

        let contexts = load_contexts().expect("load contexts");
        let session = contexts.get(&session_key("Alpha")).expect("session entry");
        let pane = session.panes.get("%1").expect("pane entry");
        assert!(pane.window_name.is_none());
        assert!(pane.window_id.is_none());
        assert!(session.windows.is_empty());
    }

    #[test]
    fn rename_session_merges_without_overwriting_existing_fields() {
        let mut env = EnvGuard::new("context-rename");
        env.set_temp_home();

        upsert_session("New".to_string(), None, None, Some("keep".to_string()))
            .expect("seed target");
        upsert_session(
            "Old".to_string(),
            Some("sid".to_string()),
            None,
            Some("old".to_string()),
        )
        .expect("seed source");

        rename_session("sid", "New").expect("rename session");

        let contexts = load_contexts().expect("load contexts");
        assert!(contexts.get(&session_key("Old")).is_none());
        let entry = contexts.get(&session_key("New")).expect("renamed entry");
        assert_eq!(entry.session_name.as_deref(), Some("New"));
        assert_eq!(entry.session_id.as_deref(), Some("sid"));
        assert_eq!(entry.session_context.as_deref(), Some("keep"));
    }

    #[test]
    fn prune_panes_removes_stale_entries() {
        let mut env = EnvGuard::new("context-prune");
        env.set_temp_home();

        upsert_pane("Alpha", "%1", None, None, None, None, None).expect("pane 1");
        upsert_pane("Alpha", "%2", None, None, None, None, None).expect("pane 2");
        upsert_pane("Beta", "%3", None, None, None, None, None).expect("pane 3");

        let mut live = HashMap::new();
        live.insert("Alpha".to_string(), HashSet::from([String::from("%1")]));

        prune_panes(&live).expect("prune panes");

        let contexts = load_contexts().expect("load contexts");
        let alpha = contexts.get(&session_key("Alpha")).expect("alpha session");
        assert!(alpha.panes.contains_key("%1"));
        assert!(!alpha.panes.contains_key("%2"));

        let beta = contexts.get(&session_key("Beta")).expect("beta session");
        assert!(beta.panes.contains_key("%3"));
    }

    #[test]
    fn sync_with_tmux_rekeys_renamed_session_and_removes_stale_data() {
        let mut env = EnvGuard::new("context-sync-rename");
        env.set_temp_home();

        upsert_session(
            "old-name".to_string(),
            Some("@1".to_string()),
            Some(AgentStatus::Working),
            Some("ctx".to_string()),
        )
        .expect("seed renamed session");
        upsert_pane(
            "old-name",
            "%1",
            None,
            None,
            None,
            None,
            Some("keep".to_string()),
        )
        .expect("pane keep");
        upsert_pane(
            "old-name",
            "%9",
            None,
            None,
            None,
            None,
            Some("drop".to_string()),
        )
        .expect("pane drop");

        upsert_session("gone".to_string(), Some("@9".to_string()), None, None)
            .expect("seed stale session");
        upsert_pane("gone", "%3", None, None, None, None, None).expect("stale pane");

        let live_sessions = vec![TmuxSession {
            id: "@1".to_string(),
            name: "new-name".to_string(),
        }];
        let live_panes = vec![TmuxPane {
            session_name: "new-name".to_string(),
            window_id: "@10".to_string(),
            window_name: "editor".to_string(),
            pane_id: "%1".to_string(),
        }];

        let summary = sync_with_tmux(&live_sessions, &live_panes).expect("sync contexts");
        assert_eq!(
            summary,
            SyncSummary {
                removed_sessions: 1,
                removed_panes: 2,
                renamed_sessions: 1,
            }
        );

        let contexts = load_contexts().expect("load contexts");
        assert!(contexts.get(&session_key("old-name")).is_none());
        assert!(contexts.get(&session_key("gone")).is_none());

        let renamed = contexts
            .get(&session_key("new-name"))
            .expect("renamed session");
        assert_eq!(renamed.session_name.as_deref(), Some("new-name"));
        assert_eq!(renamed.session_id.as_deref(), Some("@1"));
        assert!(renamed.panes.contains_key("%1"));
        assert!(!renamed.panes.contains_key("%9"));
    }

    #[test]
    fn sync_with_tmux_falls_back_to_name_when_session_id_mismatch() {
        let mut env = EnvGuard::new("context-sync-fallback-name");
        env.set_temp_home();

        upsert_session("alpha".to_string(), Some("@old".to_string()), None, None)
            .expect("seed session");
        upsert_pane("alpha", "%1", None, None, None, None, None).expect("seed pane");

        let live_sessions = vec![TmuxSession {
            id: "@new".to_string(),
            name: "alpha".to_string(),
        }];
        let live_panes = vec![TmuxPane {
            session_name: "alpha".to_string(),
            window_id: "@10".to_string(),
            window_name: "editor".to_string(),
            pane_id: "%1".to_string(),
        }];

        let summary = sync_with_tmux(&live_sessions, &live_panes).expect("sync contexts");
        assert_eq!(
            summary,
            SyncSummary {
                removed_sessions: 0,
                removed_panes: 0,
                renamed_sessions: 0,
            }
        );

        let contexts = load_contexts().expect("load contexts");
        let alpha = contexts.get(&session_key("alpha")).expect("alpha session");
        assert_eq!(alpha.session_id.as_deref(), Some("@new"));
        assert!(alpha.panes.contains_key("%1"));
    }
}
