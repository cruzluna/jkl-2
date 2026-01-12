use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            AgentStatus::Idle => "idle",
            AgentStatus::Working => "working",
            AgentStatus::Waiting => "waiting",
            AgentStatus::Done => "done",
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
pub struct SessionContext {
    pub session_name: Option<String>,
    pub status: Option<AgentStatus>,
    pub context: Option<String>,
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
    Ok(contexts)
}

pub fn upsert_context(
    session_id: String,
    session_name: String,
    status: Option<AgentStatus>,
    context: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let mut contexts = load_contexts()?;
    let entry = contexts.entry(session_id).or_default();
    entry.session_name = Some(session_name);
    if status.is_some() {
        entry.status = status;
    }
    if context.is_some() {
        entry.context = context;
    }
    save_contexts(&contexts)?;
    Ok(())
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
