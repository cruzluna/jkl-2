const statusSuggestions: Fig.Suggestion[] = [
  { name: "working", description: "Agent is actively working" },
  { name: "waiting", description: "Agent is waiting for human input" },
  { name: "done", description: "Agent has completed the task" },
  { name: "none", description: "Clear status / no explicit status" },
];

const initToolHooksSuggestions: Fig.Suggestion[] = [
  { name: "claude", description: "Initialize Claude Code hooks" },
  { name: "cursor", description: "Initialize Cursor hooks" },
  { name: "kiro", description: "Initialize Kiro CLI hooks" },
];

const initToolSkillsSuggestions: Fig.Suggestion[] = [
  { name: "codex", description: "Initialize Codex skills" },
  { name: "claude", description: "Initialize Claude skills" },
  { name: "kiro", description: "Initialize Kiro skills" },
];

const initScopeSuggestions: Fig.Suggestion[] = [
  { name: "local", description: "Write config in the current project" },
  { name: "global", description: "Write config under your HOME directory" },
];

const initPromptProviderSuggestions: Fig.Suggestion[] = [
  { name: "claude", description: "Claude Code prompts" },
  { name: "codex", description: "Codex prompts" },
  { name: "cursor", description: "Cursor CLI prompts" },
  { name: "kiro", description: "Kiro CLI prompts" },
];

const initPromptOptionSuggestions: Fig.Suggestion[] = [
  { name: "hooks", description: "Hook prompts" },
  { name: "skills", description: "Skill prompts" },
  { name: "AGENTS.md", description: "AGENTS.md prompt" },
  { name: "tmux.conf", description: "tmux.conf prompt" },
  { name: "examples", description: "Common jkl usage examples" },
];

const sessionNameGenerator: Fig.Generator = {
  script: 'tmux list-sessions -F "#{session_name}" 2>/dev/null',
  postProcess: (output) =>
    output
      .split("\n")
      .filter(Boolean)
      .map((name) => ({
        name,
        description: "tmux session",
      })),
};

const sessionIdGenerator: Fig.Generator = {
  script: 'tmux list-sessions -F "#{session_id}|#{session_name}" 2>/dev/null',
  postProcess: (output) =>
    output
      .split("\n")
      .filter(Boolean)
      .map((line) => {
        const [sessionId, sessionName] = line.split("|");
        return {
          name: sessionId,
          description: sessionName ? `tmux session: ${sessionName}` : "tmux session id",
        };
      }),
};

const paneIdGenerator: Fig.Generator = {
  script: 'tmux list-panes -a -F "#{pane_id}|#{session_name}" 2>/dev/null',
  postProcess: (output) =>
    output
      .split("\n")
      .filter(Boolean)
      .map((line) => {
        const [paneId, sessionName] = line.split("|");
        return {
          name: paneId,
          description: sessionName ? `tmux session: ${sessionName}` : "tmux pane id",
        };
      }),
};


const windowIdGenerator: Fig.Generator = {
  script: 'tmux list-windows -a -F "#{window_id}|#{window_name}|#{session_name}" 2>/dev/null',
  postProcess: (output) =>
    output
      .split("\n")
      .filter(Boolean)
      .map((line) => {
        const [windowId, windowName, sessionName] = line.split("|");
        return {
          name: windowId,
          description:
            sessionName && windowName
              ? `tmux window: ${sessionName}:${windowName}`
              : "tmux window id",
        };
      }),
};

const windowNameGenerator: Fig.Generator = {
  script: 'tmux list-windows -a -F "#{window_name}|#{session_name}" 2>/dev/null',
  postProcess: (output) =>
    output
      .split("\n")
      .filter(Boolean)
      .map((line) => {
        const [windowName, sessionName] = line.split("|");
        return {
          name: windowName,
          description: sessionName ? `tmux session: ${sessionName}` : "tmux window",
        };
      }),
};

const completionSpec: Fig.Spec = {
  name: "jkl",
  description: "Inspect agent statuses in tmux sessions",
  options: [
    {
      name: ["-h", "--help"],
      description: "Print help",
    },
    {
      name: ["-V", "--version"],
      description: "Print version",
    },
  ],
  subcommands: [
    {
      name: "tui",
      description: "Open the interactive TUI",
      options: [
        {
          name: "--session-name",
          description:
            "Session name used when opening pane status selector mode",
          args: {
            name: "session_name",
            isVariadic: true,
            generators: sessionNameGenerator,
          },
        },
        {
          name: ["--open-pane-state", "--pane-state"],
          description: "Open pane status selector popup",
        },
        {
          name: "--pane-id",
          description: "Target pane id for pane selector mode",
          args: {
            name: "pane_id",
            generators: paneIdGenerator,
          },
        },
      ],
    },
    {
      name: "upsert",
      description: "Upsert session or pane metadata",
      args: {
        name: "session_name",
        isVariadic: true,
        generators: sessionNameGenerator,
      },
      options: [
        {
          name: "--session-id",
          description: "Session id when updating a session entry",
          args: {
            name: "session_id",
            generators: sessionIdGenerator,
          },
        },
        {
          name: "--pane-id",
          description: "Pane id when updating a pane entry",
          args: {
            name: "pane_id",
            generators: paneIdGenerator,
          },
        },
        {
          name: "--window-id",
          description: "Window id when updating a pane entry",
          args: {
            name: "window_id",
            generators: windowIdGenerator,
          },
        },
        {
          name: "--window-name",
          description: "Human-friendly window name",
          args: {
            name: "window_name",
            generators: windowNameGenerator,
          },
        },
        {
          name: "--pane-name",
          description: "Human-friendly pane name",
          args: {
            name: "pane_name",
          },
        },
        {
          name: "--status",
          description: "Status value to persist",
          args: {
            name: "status",
            suggestions: statusSuggestions,
          },
        },
        {
          name: "--context",
          description: "Session or pane context text",
          args: {
            name: "context",
            isVariadic: true,
          },
        },
      ],
    },
    {
      name: "rename",
      description: "Rename a session context entry by tmux session id",
      args: [
        {
          name: "session_id",
          generators: sessionIdGenerator,
        },
        {
          name: "session_name",
          isVariadic: true,
          generators: sessionNameGenerator,
        },
      ],
    },
    {
      name: "sync",
      description: "Sync stored metadata with current tmux sessions and panes",
    },
    {
      name: "init",
      description: "Set up integrations and print copy/paste prompts",
      subcommands: [
        {
          name: "hooks",
          description: "Initialize Claude, Cursor, or Kiro hooks",
          options: [
            {
              name: "--tool",
              description: "Target integration tool",
              args: {
                name: "tool",
                suggestions: initToolHooksSuggestions,
              },
            },
            {
              name: "--scope",
              description: "Where to write hook configuration",
              args: {
                name: "scope",
                suggestions: initScopeSuggestions,
              },
            },
            {
              name: "--agent-config-dir",
              description:
                "Directory to scan for Kiro agent config files during selection (does not update all files automatically)",
              args: {
                name: "dir",
              },
            },
            {
              name: "--agent-config",
              description:
                "Explicit hook config file path(s) to update (Kiro or Cursor)",
              args: {
                name: "path",
                isVariadic: true,
              },
            },
            {
              name: "--non-interactive",
              description: "Fail instead of prompting for missing options",
            },
          ],
        },
        {
          name: "skills",
          description: "Initialize Codex, Claude, or Kiro skills",
          options: [
            {
              name: "--tool",
              description: "Target integration tool",
              args: {
                name: "tool",
                suggestions: initToolSkillsSuggestions,
              },
            },
            {
              name: "--scope",
              description: "Where to write skill configuration",
              args: {
                name: "scope",
                suggestions: initScopeSuggestions,
              },
            },
            {
              name: "--non-interactive",
              description: "Fail instead of prompting for missing options",
            },
          ],
        },
        {
          name: "prompts",
          description: "Print copy/paste prompts for integrations",
          options: [
            {
              name: "--provider",
              description: "Filter prompts by integration provider",
              args: {
                name: "provider",
                suggestions: initPromptProviderSuggestions,
              },
            },
            {
              name: "--option",
              description: "Filter prompts by prompt type",
              args: {
                name: "option",
                suggestions: initPromptOptionSuggestions,
              },
            },
          ],
        },
        {
          name: "fig-autocomplete",
          description: "Sync jkl spec into Fig and build autocomplete",
        },
      ],
    },
    {
      name: "update",
      description: "Self-update jkl binary from GitHub releases",
      options: [
        {
          name: "--pre-release",
          description: "Include master pre-release versions (-rc tags)",
        },
        {
          name: "--dev",
          description: "Use dev preview builds from the dev branch (requires --pre-release)",
        },
      ],
    },
    {
      name: "uninstall",
      description: "Uninstall jkl from the current install location",
      options: [
        {
          name: "--purge-data",
          description: "Also remove ~/.config/jkl (session metadata and logs)",
        },
      ],
    },
  ],
};

export default completionSpec;
