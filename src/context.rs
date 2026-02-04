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
    // When the Agent is working
    Working,
    // When the Agent is waiting for human intervention
    Waiting,
    // When the Agent is complete with its work
    Done,
    // When there is no status assigned to an Agent or the Agent DNE yet
    None,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct PaneContext {
    pub pane_id: Option<String>,
    pub pane_name: Option<String>,
    pub pane_status: Option<AgentStatus>,
    pub pane_context: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct SessionContext {
    pub session_name: Option<String>,
    pub session_id: Option<String>,
    pub session_status: Option<AgentStatus>,
    pub session_context: Option<String>,
    #[serde(default)]
    pub panes: PaneContextJson,
}

pub type SessionKey = String;
pub type PaneId = String;

pub type ContextJson = HashMap<SessionKey, SessionContext>;
pub type PaneContextJson = HashMap<PaneId, PaneContext>;

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
}

pub fn session_key(session_name: &str) -> String {
    blake3::hash(session_name.as_bytes()).to_hex().to_string()
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
    for (pane_id, pane) in source.panes {
        let entry = target.panes.entry(pane_id).or_default();
        if entry.pane_status.is_none() {
            entry.pane_status = pane.pane_status;
        }
        if entry.pane_context.is_none() {
            entry.pane_context = pane.pane_context;
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
