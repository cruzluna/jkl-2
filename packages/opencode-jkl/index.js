const OPENCODE_TO_JKL_STATUS = {
  busy: "working",
  retry: "working",
  idle: "waiting",
}

export const JklPlugin = async (ctx) => {
  await log(ctx, "info", "jkl OpenCode plugin initialized")

  return {
    event: async ({ event }) => {
      const status = statusFromOpenCodeEvent(event)
      if (!status) {
        return
      }

      await runJklUpsert(ctx, status)
    },
  }
}

export default JklPlugin

export function statusFromOpenCodeEvent(event) {
  if (!event || typeof event !== "object") {
    return undefined
  }

  if (event.type === "session.idle") {
    return "waiting"
  }

  if (event.type !== "session.status") {
    return undefined
  }

  const statusType = event.status?.type
  return OPENCODE_TO_JKL_STATUS[statusType]
}

export function buildJklUpsertCommand(status) {
  return `[ -n "$TMUX" ] || exit 0; command -v jkl >/dev/null 2>&1 || exit 0; jkl upsert "$(tmux display-message -p '#S')" --session-id "$(tmux display-message -p '#{session_id}')" --pane-id "$(tmux display-message -p '#{pane_id}')" --status ${status}`
}

async function runJklUpsert(ctx, status) {
  const command = buildJklUpsertCommand(status)

  try {
    await ctx.$`bash -lc ${command}`
  } catch (error) {
    await log(ctx, "warn", "jkl status sync failed", {
      status,
      error: error instanceof Error ? error.message : String(error),
    })
  }
}

async function log(ctx, level, message, extra = {}) {
  if (!ctx.client?.app?.log) {
    return
  }

  await ctx.client.app.log({
    body: {
      service: "opencode-jkl",
      level,
      message,
      extra,
    },
  })
}
