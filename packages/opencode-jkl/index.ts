import type { Plugin } from "@opencode-ai/plugin"

type JklStatus = "working" | "waiting"
type OpenCodeSessionStatus = "busy" | "retry" | "idle"
type LogLevel = "debug" | "info" | "warn" | "error"

const OPENCODE_TO_JKL_STATUS: Record<OpenCodeSessionStatus, JklStatus> = {
  busy: "working",
  retry: "working",
  idle: "waiting",
}

export const JklPlugin: Plugin = async (ctx) => {
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

export function statusFromOpenCodeEvent(event: unknown): JklStatus | undefined {
  if (!isRecord(event)) {
    return undefined
  }

  if (event.type === "session.idle") {
    return "waiting"
  }

  if (event.type !== "session.status") {
    return undefined
  }

  const propertiesStatus = isRecord(event.properties) ? event.properties.status : undefined
  const legacyStatus = event.status
  const statusType = isRecord(propertiesStatus)
    ? propertiesStatus.type
    : isRecord(legacyStatus)
      ? legacyStatus.type
      : undefined

  return isOpenCodeSessionStatus(statusType) ? OPENCODE_TO_JKL_STATUS[statusType] : undefined
}

export function buildJklUpsertCommand(status: JklStatus): string {
  return `[ -n "$TMUX" ] || exit 0; command -v jkl >/dev/null 2>&1 || exit 0; jkl upsert "$(tmux display-message -p '#S')" --session-id "$(tmux display-message -p '#{session_id}')" --pane-id "$(tmux display-message -p '#{pane_id}')" --status ${status}`
}

async function runJklUpsert(ctx: Parameters<Plugin>[0], status: JklStatus): Promise<void> {
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

async function log(
  ctx: Parameters<Plugin>[0],
  level: LogLevel,
  message: string,
  extra: Record<string, unknown> = {},
): Promise<void> {
  await ctx.client.app.log({
    body: {
      service: "opencode-jkl",
      level,
      message,
      extra,
    },
  })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}

function isOpenCodeSessionStatus(value: unknown): value is OpenCodeSessionStatus {
  return value === "busy" || value === "retry" || value === "idle"
}
