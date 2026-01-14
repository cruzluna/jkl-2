use blake3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Idle,
    Working,
    Waiting,
    Done,
    None,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            AgentStatus::Idle => "idle",
            AgentStatus::Working => "working",
            AgentStatus::Waiting => "waiting",
            AgentStatus::Done => "done",
            AgentStatus::None => "none",
        };
        formatter.write_str(text)
    }
}

impl FromStr for AgentStatus {
    type Err = StatusParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_lowercase().as_str() {
            "idle" => Ok(AgentStatus::Idle),
            "working" => Ok(AgentStatus::Working),
            "waiting" => Ok(AgentStatus::Waiting),
            "done" => Ok(AgentStatus::Done),
            "none" => Ok(AgentStatus::None),
            other => Err(StatusParseError(format!("Invalid status: {other}"))),
        }
    }
}

#[derive(Debug)]
pub struct StatusParseError(String);

impl std::fmt::Display for StatusParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for StatusParseError {}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct PaneContext {
    pub status: Option<AgentStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct SessionContext {
    pub session_name: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    pub status: Option<AgentStatus>,
    pub context: Option<String>,
    #[serde(default)]
    pub panes: HashMap<String, PaneContext>,
}

pub fn session_key(session_name: &str) -> String {
    blake3::hash(session_name.as_bytes()).to_hex().to_string()
}

pub fn load_contexts() -> Result<HashMap<String, SessionContext>, Box<dyn Error>> {
    let Some(path) = context_path() else {
        return Ok(HashMap::new());
    };
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, "{}")?;
            "{}".to_string()
        }
        Err(error) => return Err(Box::new(error)),
    };
    let contexts = serde_json::from_str(&contents)?;
    Ok(normalize_context_keys(contexts))
}

pub fn upsert_session(
    session_name: String,
    session_id: Option<String>,
    status: Option<AgentStatus>,
    context: Option<String>,
) -> Result<String, Box<dyn Error>> {
    let mut contexts = load_contexts()?;
    let key = session_key(&session_name);
    let entry = contexts.entry(key.clone()).or_default();
    entry.session_name = Some(session_name);
    if let Some(session_id) = session_id {
        entry.session_id = Some(session_id);
    }
    if status.is_some() {
        entry.status = status;
    }
    if context.is_some() {
        entry.context = context;
    }
    save_contexts(&contexts)?;
    Ok(key)
}

pub fn upsert_pane(
    session_name: &str,
    pane_id: &str,
    status: Option<AgentStatus>,
) -> Result<(), Box<dyn Error>> {
    let mut contexts = load_contexts()?;
    let key = session_key(session_name);
    let entry = contexts.entry(key).or_default();
    entry.session_name = Some(session_name.to_string());
    let pane = entry.panes.entry(pane_id.to_string()).or_default();
    pane.status = status;
    save_contexts(&contexts)?;
    Ok(())
}

pub fn rename_session(session_id: &str, session_name: &str) -> Result<(), Box<dyn Error>> {
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
    let mut contexts = load_contexts()?;
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
    save_contexts(&contexts)?;
    Ok(())
}

fn normalize_context_keys(
    contexts: HashMap<String, SessionContext>,
) -> HashMap<String, SessionContext> {
    let mut normalized = HashMap::new();
    for (key, context) in contexts {
        let normalized_key = context
            .session_name
            .as_deref()
            .map(session_key)
            .unwrap_or(key);
        let entry = normalized.entry(normalized_key).or_default();
        merge_context(entry, context);
    }
    normalized
}

fn merge_context(target: &mut SessionContext, source: SessionContext) {
    if target.session_name.is_none() {
        target.session_name = source.session_name;
    }
    if target.session_id.is_none() {
        target.session_id = source.session_id;
    }
    if target.status.is_none() {
        target.status = source.status;
    }
    if target.context.is_none() {
        target.context = source.context;
    }
    for (pane_id, pane) in source.panes {
        let entry = target.panes.entry(pane_id).or_default();
        if entry.status.is_none() {
            entry.status = pane.status;
        }
    }
}

fn save_contexts(contexts: &HashMap<String, SessionContext>) -> Result<(), Box<dyn Error>> {
    let Some(path) = context_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(contexts)?;
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, contents)?;
    fs::rename(&temp_path, &path)?;
    Ok(())
}

fn context_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let base_dir = PathBuf::from(home).join(".config");
    Some(base_dir.join("jkl").join("session_context.json"))
}
