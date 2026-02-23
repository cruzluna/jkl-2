use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use inquire::{Select, Text};
use serde_json::{Map, Value, json};
use std::fmt;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

const CLAUDE_WORKING_COMMAND: &str = "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status working";
const CLAUDE_WAITING_COMMAND: &str = "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status waiting";
const KIRO_WORKING_COMMAND: &str = "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status working";
const KIRO_WAITING_COMMAND: &str = "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status waiting";
const CURSOR_WORKING_COMMAND: &str = "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status working";
const CURSOR_WAITING_COMMAND: &str = "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status waiting";

const KIRO_NAME: &str = "jkl";
const KIRO_DESCRIPTION: &str = "Sync jkl status with Kiro activity";

const JKL_SKILL_MD: &str = include_str!("../skills/jkl-cli/SKILL.md");
const JKL_SKILL_OPENAI_YAML: &str = include_str!("../skills/jkl-cli/agents/openai.yaml");
const FIG_SPEC: &str = include_str!("../completions/fig/jkl.ts");
const BASIC_INSTRUCTIONS: &str = r#"## Basic JKL Instructions

- Do not invoke the TUI command (`jkl tui`) from automated agents.
- Upsert session metadata: `jkl upsert <session_name...> [--session-id <session_id>] [--status <status>] [--context <text...>]`
- Upsert pane metadata: `jkl upsert <session_name...> --pane-id <pane_id> [--window-id <window_id> [--window-name <window_name>]] [--status <status>] [--context <text...>]`
- Rename a session entry: `jkl rename <session_id> <session_name...>`
- Sync persisted metadata with live tmux state: `jkl sync`
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum InitTool {
    Claude,
    Cursor,
    Kiro,
    Codex,
}

impl fmt::Display for InitTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kiro => write!(f, "kiro"),
            Self::Claude => write!(f, "claude"),
            Self::Cursor => write!(f, "cursor"),
            Self::Codex => write!(f, "codex"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum InitScope {
    Local,
    Global,
}

impl fmt::Display for InitScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Global => write!(f, "global"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitTarget {
    Hooks,
    Skills,
    FigAutocomplete,
    AgentsMd,
}

impl fmt::Display for InitTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hooks => write!(f, "hooks"),
            Self::Skills => write!(f, "skills"),
            Self::FigAutocomplete => write!(f, "fig-autocomplete"),
            Self::AgentsMd => write!(f, "agents-md"),
        }
    }
}

pub fn run_interactive() -> Result<()> {
    ensure_interactive_terminal()?;

    let target = Select::new(
        "What do you want to initialize?",
        vec![
            InitTarget::Hooks,
            InitTarget::Skills,
            InitTarget::FigAutocomplete,
            InitTarget::AgentsMd,
        ],
    )
    .prompt()
    .context("prompt for init target")?;

    match target {
        InitTarget::Hooks => run_hooks(None, None, None, None, false),
        InitTarget::Skills => run_skills(None, None, false),
        InitTarget::FigAutocomplete => run_fig_autocomplete(),
        InitTarget::AgentsMd => run_agents_md(std::env::current_dir()?),
    }
}

pub fn run_hooks(
    tool: Option<InitTool>,
    scope: Option<InitScope>,
    agents_dir: Option<PathBuf>,
    config_paths: Option<Vec<PathBuf>>,
    non_interactive: bool,
) -> Result<()> {
    let tool = resolve_tool(
        tool,
        non_interactive,
        &[InitTool::Claude, InitTool::Cursor, InitTool::Kiro],
        "Select the tool to initialize hooks for:",
        "--tool is required when using --non-interactive",
    )?;
    let scope = resolve_scope(scope, non_interactive)?;

    match tool {
        InitTool::Claude => {
            if config_paths.is_some() {
                bail!("--agent-config is only supported for --tool kiro or cursor");
            }
            if agents_dir.is_some() {
                bail!("--agent-config-dir is only supported for --tool kiro");
            }
            let path = hooks_config_path(tool, scope)?;
            apply_hooks_to_file(&path, tool)?;
        }
        InitTool::Kiro => {
            let paths = resolve_kiro_hook_paths(scope, agents_dir, config_paths, non_interactive)?;
            if paths.is_empty() {
                println!("No Kiro agent configs selected. No files updated.");
                return Ok(());
            }
            for path in paths {
                apply_hooks_to_file(&path, tool)?;
            }
        }
        InitTool::Cursor => {
            if agents_dir.is_some() {
                bail!("--agent-config-dir is only supported for --tool kiro");
            }
            let paths = resolve_cursor_hook_paths(scope, config_paths)?;
            for path in paths {
                apply_hooks_to_file(&path, tool)?;
            }
        }
        InitTool::Codex => bail!("codex does not support hooks"),
    }

    Ok(())
}

pub fn run_skills(
    tool: Option<InitTool>,
    scope: Option<InitScope>,
    non_interactive: bool,
) -> Result<()> {
    let tool = resolve_tool(
        tool,
        non_interactive,
        &[InitTool::Codex, InitTool::Claude, InitTool::Kiro],
        "Select the tool to initialize skills for:",
        "--tool is required when using --non-interactive",
    )?;
    let scope = resolve_scope(scope, non_interactive)?;

    let root = skills_root_path(tool, scope)?;
    let skill_dir = root.join("jkl-cli");
    let skill_md = skill_dir.join("SKILL.md");
    let agent_yaml = skill_dir.join("agents").join("openai.yaml");

    let mut changed_files = 0;
    changed_files += usize::from(write_file_if_changed(&skill_md, JKL_SKILL_MD.as_bytes())?);
    changed_files += usize::from(write_file_if_changed(
        &agent_yaml,
        JKL_SKILL_OPENAI_YAML.as_bytes(),
    )?);

    if changed_files > 0 {
        println!(
            "Initialized jkl skill for {} at {}",
            tool,
            skill_dir.display()
        );
    } else {
        println!("Skill already up to date at {}", skill_dir.display());
    }

    Ok(())
}

pub fn run_fig_autocomplete() -> Result<()> {
    let fig_repo_dir = ensure_fig_repo_dir()?;

    if !command_in_path("npm") {
        bail!("npm is required but was not found in PATH");
    }

    let spec_dest = fig_repo_dir.join("src").join("jkl.ts");
    write_file_if_changed(&spec_dest, FIG_SPEC.as_bytes())?;
    println!("Synced spec to: {}", spec_dest.display());

    run_command(
        Command::new("npm")
            .arg("run")
            .arg("build")
            .current_dir(&fig_repo_dir),
        "build Fig autocomplete",
    )?;

    println!("Fig autocomplete build complete.");
    Ok(())
}

pub fn run_agents_md(root: PathBuf) -> Result<()> {
    let summary = append_basic_instructions(&root)?;
    println!(
        "Updated {} AGENTS.md files (skipped {})",
        summary.files_updated, summary.files_skipped
    );
    Ok(())
}

fn resolve_tool(
    tool: Option<InitTool>,
    non_interactive: bool,
    allowed_tools: &[InitTool],
    prompt: &str,
    missing_tool_message: &str,
) -> Result<InitTool> {
    if let Some(tool) = tool {
        if allowed_tools.contains(&tool) {
            return Ok(tool);
        }
        bail!("unsupported tool '{}' for this init command", tool);
    }

    if non_interactive {
        bail!("{missing_tool_message}");
    }

    ensure_interactive_terminal()?;
    Select::new(prompt, allowed_tools.to_vec())
        .prompt()
        .context("prompt for tool selection")
}

fn resolve_scope(scope: Option<InitScope>, non_interactive: bool) -> Result<InitScope> {
    if let Some(scope) = scope {
        return Ok(scope);
    }

    if non_interactive {
        bail!("--scope is required when using --non-interactive");
    }

    ensure_interactive_terminal()?;
    Select::new(
        "Select where to initialize:",
        vec![InitScope::Local, InitScope::Global],
    )
    .prompt()
    .context("prompt for scope selection")
}

fn apply_hooks_to_file(path: &Path, tool: InitTool) -> Result<()> {
    let existed_before = path.exists();
    let mut root = load_json_object_or_empty(path)?;
    let changed = match tool {
        InitTool::Claude => ensure_claude_hooks(&mut root)?,
        InitTool::Cursor => ensure_cursor_hooks(&mut root)?,
        InitTool::Kiro => ensure_kiro_hooks(&mut root)?,
        InitTool::Codex => bail!("codex does not support hooks"),
    };

    if changed || !existed_before {
        write_json_pretty(path, &root)?;
        println!("Initialized hooks at {}", path.display());
    } else {
        println!("Hooks already configured at {}", path.display());
    }

    Ok(())
}

fn hooks_config_path(tool: InitTool, scope: InitScope) -> Result<PathBuf> {
    let home = home_dir()?;

    Ok(match (tool, scope) {
        (InitTool::Claude, InitScope::Global) => home.join(".claude").join("settings.json"),
        (InitTool::Claude, InitScope::Local) => std::env::current_dir()?
            .join(".claude")
            .join("settings.local.json"),
        (InitTool::Cursor, InitScope::Global) => home.join(".cursor").join("hooks.json"),
        (InitTool::Cursor, InitScope::Local) => {
            std::env::current_dir()?.join(".cursor").join("hooks.json")
        }
        (InitTool::Kiro, InitScope::Global) => home.join(".kiro").join("agents").join("jkl.json"),
        (InitTool::Kiro, InitScope::Local) => std::env::current_dir()?
            .join(".kiro")
            .join("agents")
            .join("jkl.json"),
        (InitTool::Codex, _) => bail!("codex does not support hooks"),
    })
}

fn kiro_agents_dir(scope: InitScope) -> Result<PathBuf> {
    let home = home_dir()?;
    let cwd = std::env::current_dir()?;
    Ok(match scope {
        InitScope::Local => cwd.join(".kiro").join("agents"),
        InitScope::Global => home.join(".kiro").join("agents"),
    })
}

fn resolve_kiro_hook_paths(
    scope: InitScope,
    agents_dir_override: Option<PathBuf>,
    config_paths: Option<Vec<PathBuf>>,
    non_interactive: bool,
) -> Result<Vec<PathBuf>> {
    if let Some(paths) = config_paths {
        let normalized = normalize_user_paths(paths)?;
        if normalized.is_empty() {
            bail!("at least one --config path is required");
        }
        return Ok(normalized);
    }

    let agents_dir = resolve_kiro_agents_dir(scope, agents_dir_override, non_interactive)?;

    if non_interactive {
        return Ok(vec![agents_dir.join("jkl.json")]);
    }

    ensure_interactive_terminal()?;
    let detected = discover_kiro_agent_configs(&agents_dir)?;
    if detected.is_empty() {
        return Ok(Vec::new());
    }

    select_kiro_configs_with_enter(detected)
}

fn resolve_cursor_hook_paths(
    scope: InitScope,
    config_paths: Option<Vec<PathBuf>>,
) -> Result<Vec<PathBuf>> {
    if let Some(paths) = config_paths {
        return normalize_user_paths(paths);
    }

    Ok(vec![hooks_config_path(InitTool::Cursor, scope)?])
}

fn discover_kiro_agent_configs(agents_dir: &Path) -> Result<Vec<PathChoice>> {
    if !agents_dir.exists() {
        return Ok(Vec::new());
    }

    let mut choices = Vec::new();
    for entry in fs::read_dir(agents_dir)
        .with_context(|| format!("read Kiro agents directory {}", agents_dir.display()))?
    {
        let entry = entry.context("read directory entry")?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.json");
        choices.push(PathChoice {
            label: format!("{} ({})", file_name, path.display()),
            path,
        });
    }
    choices.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(choices)
}

fn select_kiro_configs_with_enter(detected: Vec<PathChoice>) -> Result<Vec<PathBuf>> {
    let mut remaining = detected;
    let mut selected = Vec::new();

    loop {
        let mut options: Vec<String> = remaining
            .iter()
            .map(|choice| choice.label.clone())
            .collect();
        options.push("Finish selection".to_string());

        let prompt = format!(
            "Select Kiro agent config file (selected: {}, Enter to choose):",
            selected.len()
        );
        let picked = Select::new(prompt.as_str(), options)
            .prompt()
            .context("prompt for Kiro config file selection")?;

        if picked == "Finish selection" {
            break;
        }

        if let Some(index) = remaining.iter().position(|choice| choice.label == picked) {
            let choice = remaining.remove(index);
            selected.push(choice.path);
        }

        if remaining.is_empty() {
            break;
        }
    }

    Ok(selected)
}

fn resolve_kiro_agents_dir(
    scope: InitScope,
    override_dir: Option<PathBuf>,
    non_interactive: bool,
) -> Result<PathBuf> {
    let default_dir = kiro_agents_dir(scope)?;
    if let Some(dir) = override_dir {
        return absolutize_path(dir);
    }

    if non_interactive {
        return Ok(default_dir);
    }

    ensure_interactive_terminal()?;
    let raw = Text::new(&format!(
        "Agent config directory [{}]:",
        default_dir.display()
    ))
    .with_help_message("specify override")
    .prompt()
    .context("prompt for Kiro agents directory")?;

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(default_dir);
    }
    absolutize_path(PathBuf::from(trimmed))
}

fn absolutize_path(path: PathBuf) -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("resolve current working directory")?;
    Ok(absolutize_from(path, &cwd))
}

fn absolutize_from(path: PathBuf, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn normalize_user_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let cwd = std::env::current_dir().context("resolve current working directory")?;
    normalize_user_paths_from(paths, &cwd)
}

fn normalize_user_paths_from(paths: Vec<PathBuf>, cwd: &Path) -> Result<Vec<PathBuf>> {
    let mut normalized = Vec::new();
    for path in paths {
        if path.as_os_str().is_empty() {
            continue;
        }
        let resolved = absolutize_from(path, cwd);
        if !normalized.iter().any(|existing| existing == &resolved) {
            normalized.push(resolved);
        }
    }
    if normalized.is_empty() {
        bail!("at least one config path is required");
    }
    Ok(normalized)
}

fn skills_root_path(tool: InitTool, scope: InitScope) -> Result<PathBuf> {
    let home = home_dir()?;
    let cwd = std::env::current_dir()?;

    Ok(match (tool, scope) {
        (InitTool::Codex, InitScope::Local) => cwd.join(".agents").join("skills"),
        (InitTool::Codex, InitScope::Global) => home.join(".agents").join("skills"),
        (InitTool::Claude, InitScope::Local) => cwd.join(".claude").join("skills"),
        (InitTool::Claude, InitScope::Global) => home.join(".claude").join("skills"),
        (InitTool::Cursor, _) => bail!("cursor does not support skills"),
        (InitTool::Kiro, InitScope::Local) => cwd.join(".kiro").join("skills"),
        (InitTool::Kiro, InitScope::Global) => home.join(".kiro").join("skills"),
    })
}

fn load_json_object_or_empty(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("read JSON config at {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }

    let parsed: Value = serde_json::from_str(&content)
        .with_context(|| format!("parse JSON config at {}", path.display()))?;
    if !parsed.is_object() {
        bail!("expected JSON object at {}", path.display());
    }

    Ok(parsed)
}

fn write_json_pretty(path: &Path, value: &Value) -> Result<()> {
    let parent = path
        .parent()
        .context("resolve config file parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create config directory {}", parent.display()))?;

    let serialized = serde_json::to_string_pretty(value).context("serialize JSON config")?;
    fs::write(path, format!("{serialized}\n"))
        .with_context(|| format!("write JSON config at {}", path.display()))
}

fn ensure_claude_hooks(root: &mut Value) -> Result<bool> {
    let mut changed = false;
    changed |= ensure_claude_hook(root, "UserPromptSubmit", CLAUDE_WORKING_COMMAND)?;
    changed |= ensure_claude_hook(root, "Stop", CLAUDE_WAITING_COMMAND)?;
    Ok(changed)
}

fn ensure_claude_hook(root: &mut Value, event: &str, command: &str) -> Result<bool> {
    let root_obj = root
        .as_object_mut()
        .context("claude config root must be a JSON object")?;
    let hooks = object_field(root_obj, "hooks")?;
    let event_entries = array_field(hooks, event)?;

    if claude_event_contains_command(event_entries, command) {
        return Ok(false);
    }

    event_entries.push(json!({
        "hooks": [
            {
                "type": "command",
                "command": command
            }
        ]
    }));

    Ok(true)
}

fn claude_event_contains_command(entries: &[Value], command: &str) -> bool {
    entries.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|hooks| {
                hooks.iter().any(|hook| {
                    hook.get("type").and_then(Value::as_str) == Some("command")
                        && hook.get("command").and_then(Value::as_str) == Some(command)
                })
            })
    })
}

fn ensure_kiro_hooks(root: &mut Value) -> Result<bool> {
    let root_obj = root
        .as_object_mut()
        .context("kiro config root must be a JSON object")?;

    let mut changed = false;
    if !root_obj.contains_key("name") {
        root_obj.insert("name".to_string(), Value::String(KIRO_NAME.to_string()));
        changed = true;
    }
    if !root_obj.contains_key("description") {
        root_obj.insert(
            "description".to_string(),
            Value::String(KIRO_DESCRIPTION.to_string()),
        );
        changed = true;
    }

    let hooks = object_field(root_obj, "hooks")?;
    changed |= ensure_kiro_hook_event(hooks, "userPromptSubmit", KIRO_WORKING_COMMAND)?;
    changed |= ensure_kiro_hook_event(hooks, "stop", KIRO_WAITING_COMMAND)?;

    Ok(changed)
}

fn ensure_kiro_hook_event(
    hooks: &mut Map<String, Value>,
    event: &str,
    command: &str,
) -> Result<bool> {
    let event_entries = array_field(hooks, event)?;

    if event_entries
        .iter()
        .any(|entry| entry.get("command").and_then(Value::as_str) == Some(command))
    {
        return Ok(false);
    }

    event_entries.push(json!({ "command": command }));
    Ok(true)
}

fn ensure_cursor_hooks(root: &mut Value) -> Result<bool> {
    let root_obj = root
        .as_object_mut()
        .context("cursor config root must be a JSON object")?;

    let mut changed = false;
    if !root_obj.contains_key("version") {
        root_obj.insert("version".to_string(), json!(1));
        changed = true;
    }

    let hooks = object_field(root_obj, "hooks")?;
    changed |= ensure_cursor_hook_event(hooks, "beforeSubmitPrompt", CURSOR_WORKING_COMMAND)?;
    changed |= ensure_cursor_hook_event(hooks, "stop", CURSOR_WAITING_COMMAND)?;
    Ok(changed)
}

fn ensure_cursor_hook_event(
    hooks: &mut Map<String, Value>,
    event: &str,
    command: &str,
) -> Result<bool> {
    let event_entries = array_field(hooks, event)?;

    if event_entries
        .iter()
        .any(|entry| entry.get("command").and_then(Value::as_str) == Some(command))
    {
        return Ok(false);
    }

    event_entries.push(json!({ "command": command }));
    Ok(true)
}

fn object_field<'a>(
    object: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>> {
    let value = object
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    value
        .as_object_mut()
        .with_context(|| format!("'{}' must be a JSON object", key))
}

fn array_field<'a>(object: &'a mut Map<String, Value>, key: &str) -> Result<&'a mut Vec<Value>> {
    let value = object
        .entry(key.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    value
        .as_array_mut()
        .with_context(|| format!("'{}' must be a JSON array", key))
}

fn write_file_if_changed(path: &Path, content: &[u8]) -> Result<bool> {
    if let Ok(existing) = fs::read(path) {
        if existing == content {
            return Ok(false);
        }
    }

    let parent = path
        .parent()
        .context("resolve destination file parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create destination directory {}", parent.display()))?;
    fs::write(path, content).with_context(|| format!("write file {}", path.display()))?;
    Ok(true)
}

fn append_basic_instructions(root: &Path) -> Result<AppendSummary> {
    let mut summary = AppendSummary::default();
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };

        for entry in entries {
            let entry = entry.context("read directory entry")?;
            let entry_path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("read file type for {}", entry_path.display()))?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry_path);
                continue;
            }
            if !file_type.is_file() || entry.file_name() != "AGENTS.md" {
                continue;
            }
            if append_instructions_to_file(&entry_path)? {
                summary.files_updated += 1;
            } else {
                summary.files_skipped += 1;
            }
        }
    }

    Ok(summary)
}

fn append_instructions_to_file(path: &Path) -> Result<bool> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("read AGENTS.md file at {}", path.display()))?;
    if contents.contains("## Basic JKL Instructions") {
        return Ok(false);
    }

    let mut next = String::with_capacity(contents.len() + BASIC_INSTRUCTIONS.len() + 2);
    let trimmed = contents.trim_end();
    if !trimmed.is_empty() {
        next.push_str(trimmed);
        next.push_str("\n\n");
    }
    next.push_str(BASIC_INSTRUCTIONS);
    if !next.ends_with('\n') {
        next.push('\n');
    }

    fs::write(path, next).with_context(|| format!("write AGENTS.md file at {}", path.display()))?;
    Ok(true)
}

fn ensure_fig_repo_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    let default_dir = home.join(".fig").join("autocomplete");
    let fallback_dir = home.join(".fig").join(".fig").join("autocomplete");

    let env_override = std::env::var_os("FIG_AUTOCOMPLETE_DIR");
    let mut repo_dir = env_override
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_dir.clone());

    if repo_dir.is_dir() {
        return Ok(repo_dir);
    }

    if !command_in_path("npx") {
        bail!(
            "Fig autocomplete repo not found at {} and npx is required to initialize it",
            repo_dir.display()
        );
    }

    let mut init_dir = repo_dir
        .parent()
        .context("resolve Fig autocomplete repository parent directory")?;
    if env_override.is_none() && repo_dir == default_dir {
        init_dir = &home;
    }
    fs::create_dir_all(init_dir)
        .with_context(|| format!("create Fig parent directory {}", init_dir.display()))?;

    run_command(
        Command::new("npx")
            .arg("@withfig/autocomplete-tools@latest")
            .arg("init")
            .current_dir(init_dir),
        "initialize Fig autocomplete repository",
    )?;

    if repo_dir.is_dir() {
        return Ok(repo_dir);
    }

    if env_override.is_none() && fallback_dir.is_dir() {
        repo_dir = fallback_dir;
        return Ok(repo_dir);
    }

    bail!(
        "failed to initialize Fig autocomplete repository at {}",
        repo_dir.display()
    )
}

fn command_in_path(command: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(command).is_file())
}

fn run_command(command: &mut Command, action: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("{}: start command", action))?;
    if status.success() {
        return Ok(());
    }

    bail!("{}: command exited with {}", action, status)
}

fn ensure_interactive_terminal() -> Result<()> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        return Ok(());
    }

    bail!("interactive init requires a terminal; use --non-interactive with explicit flags")
}

fn home_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home))
}

#[derive(Clone, Debug)]
struct PathChoice {
    label: String,
    path: PathBuf,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct AppendSummary {
    files_updated: usize,
    files_skipped: usize,
}

impl fmt::Display for PathChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;

    #[test]
    fn skills_root_for_codex_global_uses_home_agents_skills() {
        let mut env = EnvGuard::new("init-skills-root-codex-global");
        let home = env.set_temp_home();

        let root = skills_root_path(InitTool::Codex, InitScope::Global).expect("skills root");
        assert_eq!(root, home.join(".agents").join("skills"));
    }

    #[test]
    fn hooks_path_rejects_codex() {
        let mut env = EnvGuard::new("init-hooks-codex-reject");
        env.set_temp_home();

        let err = hooks_config_path(InitTool::Codex, InitScope::Local).expect_err("expected error");
        assert!(err.to_string().contains("does not support hooks"));
    }

    #[test]
    fn hooks_path_for_cursor_resolves_global_and_local() {
        let mut env = EnvGuard::new("init-hooks-cursor-paths");
        let home = env.set_temp_home();
        let cwd = env.temp_dir().join("project");
        fs::create_dir_all(&cwd).expect("create cwd");
        let previous_cwd = std::env::current_dir().expect("current cwd");
        struct CwdGuard(PathBuf);
        impl Drop for CwdGuard {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
            }
        }
        let _cwd_guard = CwdGuard(previous_cwd);
        std::env::set_current_dir(&cwd).expect("set cwd");

        let global = hooks_config_path(InitTool::Cursor, InitScope::Global).expect("global path");
        let local = hooks_config_path(InitTool::Cursor, InitScope::Local).expect("local path");

        assert_eq!(global, home.join(".cursor").join("hooks.json"));
        let cwd_resolved = std::env::current_dir().expect("resolved cwd");
        assert_eq!(local, cwd_resolved.join(".cursor").join("hooks.json"));
    }

    #[test]
    fn ensure_claude_hooks_is_idempotent() {
        let mut root = json!({});

        let first = ensure_claude_hooks(&mut root).expect("first ensure");
        let second = ensure_claude_hooks(&mut root).expect("second ensure");

        assert!(first);
        assert!(!second);

        let hooks = root
            .get("hooks")
            .and_then(Value::as_object)
            .expect("hooks object");
        let submit = hooks
            .get("UserPromptSubmit")
            .and_then(Value::as_array)
            .expect("submit array");
        assert_eq!(submit.len(), 1);
    }

    #[test]
    fn ensure_kiro_hooks_is_idempotent() {
        let mut root = json!({});

        let first = ensure_kiro_hooks(&mut root).expect("first ensure");
        let second = ensure_kiro_hooks(&mut root).expect("second ensure");

        assert!(first);
        assert!(!second);

        let hooks = root
            .get("hooks")
            .and_then(Value::as_object)
            .expect("hooks object");
        let stop = hooks
            .get("stop")
            .and_then(Value::as_array)
            .expect("stop array");
        assert_eq!(stop.len(), 1);
    }

    #[test]
    fn ensure_cursor_hooks_is_idempotent() {
        let mut root = json!({});

        let first = ensure_cursor_hooks(&mut root).expect("first ensure");
        let second = ensure_cursor_hooks(&mut root).expect("second ensure");

        assert!(first);
        assert!(!second);

        let version = root
            .get("version")
            .and_then(Value::as_i64)
            .expect("version");
        assert_eq!(version, 1);

        let hooks = root
            .get("hooks")
            .and_then(Value::as_object)
            .expect("hooks object");
        let submit = hooks
            .get("beforeSubmitPrompt")
            .and_then(Value::as_array)
            .expect("beforeSubmitPrompt array");
        assert_eq!(submit.len(), 1);
    }

    #[test]
    fn write_file_if_changed_skips_unchanged() {
        let env = EnvGuard::new("init-write-file-if-changed");
        let path = env.temp_dir().join("file.txt");

        let first = write_file_if_changed(&path, b"hello").expect("first write");
        let second = write_file_if_changed(&path, b"hello").expect("second write");

        assert!(first);
        assert!(!second);
    }

    #[test]
    fn normalize_user_paths_deduplicates_and_resolves_relative_paths() {
        let env = EnvGuard::new("init-normalize-paths");
        let cwd = env.temp_dir().join("project");
        fs::create_dir_all(&cwd).expect("create cwd");

        let paths = vec![
            PathBuf::from(".kiro/agents/a.json"),
            PathBuf::from(".kiro/agents/a.json"),
            PathBuf::from("/tmp/absolute.json"),
        ];

        let normalized = normalize_user_paths_from(paths, &cwd).expect("normalize");
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0], cwd.join(".kiro/agents/a.json"));
        assert_eq!(normalized[1], PathBuf::from("/tmp/absolute.json"));
    }

    #[test]
    fn absolutize_from_keeps_absolute_and_resolves_relative() {
        let cwd = PathBuf::from("/tmp/root");
        assert_eq!(
            absolutize_from(PathBuf::from("a/b.json"), &cwd),
            PathBuf::from("/tmp/root/a/b.json")
        );
        assert_eq!(
            absolutize_from(PathBuf::from("/etc/test.json"), &cwd),
            PathBuf::from("/etc/test.json")
        );
    }

    #[test]
    fn resolve_kiro_hook_paths_non_interactive_uses_override_directory() {
        let env = EnvGuard::new("init-kiro-dir-override");
        let override_dir = env.temp_dir().join("custom").join("agents");
        let paths =
            resolve_kiro_hook_paths(InitScope::Local, Some(override_dir.clone()), None, true)
                .expect("resolve paths");
        assert_eq!(paths, vec![override_dir.join("jkl.json")]);
    }

    #[test]
    fn resolve_cursor_hook_paths_uses_explicit_paths() {
        let env = EnvGuard::new("init-cursor-path-override");
        let cwd = env.temp_dir().join("project");
        fs::create_dir_all(&cwd).expect("create cwd");
        let explicit = cwd.join(".cursor").join("dev-hooks.json");

        let resolved = resolve_cursor_hook_paths(InitScope::Local, Some(vec![explicit.clone()]))
            .expect("resolve cursor paths");
        assert_eq!(resolved, vec![explicit]);
    }

    #[test]
    fn append_basic_instructions_updates_nested_agents_files() {
        let env = EnvGuard::new("init-agents-md-updates-nested");
        let root = env.temp_dir().join("workspace");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("create nested directory");
        let root_file = root.join("AGENTS.md");
        let nested_file = nested.join("AGENTS.md");
        fs::write(&root_file, "# Root\n").expect("write root AGENTS.md");
        fs::write(&nested_file, "# Nested\n").expect("write nested AGENTS.md");

        let summary = append_basic_instructions(&root).expect("append instructions");

        assert_eq!(
            summary,
            AppendSummary {
                files_updated: 2,
                files_skipped: 0
            }
        );
        let root_contents = fs::read_to_string(&root_file).expect("read root AGENTS.md");
        let nested_contents = fs::read_to_string(&nested_file).expect("read nested AGENTS.md");
        assert!(root_contents.contains("## Basic JKL Instructions"));
        assert!(nested_contents.contains("## Basic JKL Instructions"));
    }

    #[test]
    fn append_basic_instructions_skips_existing_section() {
        let env = EnvGuard::new("init-agents-md-skip-existing");
        let root = env.temp_dir().join("workspace");
        fs::create_dir_all(&root).expect("create root directory");
        let agents_file = root.join("AGENTS.md");
        fs::write(&agents_file, BASIC_INSTRUCTIONS).expect("write baseline instructions");

        let summary = append_basic_instructions(&root).expect("append instructions");

        assert_eq!(
            summary,
            AppendSummary {
                files_updated: 0,
                files_skipped: 1
            }
        );
    }
}
