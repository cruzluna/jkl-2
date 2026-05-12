import { execFile } from "node:child_process"
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent"

type JklStatus = "working" | "waiting"
type RunCommand = (file: string, args: readonly string[], callback: (error: Error | null) => void) => void

export default function jklPiExtension(pi: ExtensionAPI): void {
  pi.on("session_start", async (_event, ctx) => {
    await syncAndUpdateUi("waiting", ctx)
  })

  pi.on("agent_start", async (_event, ctx) => {
    await syncAndUpdateUi("working", ctx)
  })

  pi.on("agent_end", async (_event, ctx) => {
    await syncAndUpdateUi("waiting", ctx)
  })

  pi.on("session_shutdown", async (_event, ctx) => {
    await syncAndUpdateUi("waiting", ctx)
  })
}

export function buildJklUpsertCommand(status: JklStatus): string {
  return `[ -n "$TMUX" ] || exit 0; command -v jkl >/dev/null 2>&1 || exit 0; jkl upsert "$(tmux display-message -p '#S')" --session-id "$(tmux display-message -p '#{session_id}')" --pane-id "$(tmux display-message -p '#{pane_id}')" --status ${status}`
}

export async function syncJklStatus(status: JklStatus, run: RunCommand = runCommand): Promise<void> {
  const command = buildJklUpsertCommand(status)

  await new Promise<void>((resolve, reject) => {
    run("bash", ["-lc", command], (error) => {
      if (error) {
        reject(error)
        return
      }
      resolve()
    })
  })
}

const runCommand: RunCommand = (file, args, callback) => {
  execFile(file, [...args], (error) => callback(error))
}

async function syncAndUpdateUi(
  status: JklStatus,
  ctx: { ui: { setStatus(key: string, text: string | undefined): void } },
): Promise<void> {
  ctx.ui.setStatus("jkl", `jkl: ${status}`)
  try {
    await syncJklStatus(status)
  } catch {
    ctx.ui.setStatus("jkl", "jkl: sync failed")
  }
}
