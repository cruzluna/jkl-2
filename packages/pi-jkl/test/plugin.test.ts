import assert from "node:assert/strict"
import test from "node:test"
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent"

import jklPiExtension, { buildJklUpsertCommand, syncJklStatus } from "../index.ts"

test("builds a tmux-aware jkl upsert command", () => {
  const command = buildJklUpsertCommand("working")

  assert.match(command, /\[ -n "\$TMUX" \] \|\| exit 0/)
  assert.match(command, /command -v jkl/)
  assert.match(command, /jkl upsert/)
  assert.match(command, /#\{session_id\}/)
  assert.match(command, /#\{pane_id\}/)
  assert.match(command, /--status working/)
})

test("syncJklStatus runs through bash", async () => {
  const calls: Array<{ file: string; args: string[] }> = []

  await syncJklStatus("waiting", ((file, args, callback) => {
    calls.push({ file, args: args as string[] })
    callback(null)
  }) as Parameters<typeof syncJklStatus>[1])

  assert.deepEqual(calls, [
    {
      file: "bash",
      args: ["-lc", buildJklUpsertCommand("waiting")],
    },
  ])
})

test("extension registers Pi lifecycle status handlers", async () => {
  const handlers = new Map<string, (event: unknown, ctx: MockContext) => Promise<void>>()
  const pi = {
    on: (event: string, handler: (event: unknown, ctx: MockContext) => Promise<void>) => {
      handlers.set(event, handler)
    },
  }
  const statuses: Array<string | undefined> = []
  const ctx: MockContext = {
    ui: {
      setStatus: (_key, text) => {
        statuses.push(text)
      },
    },
  }

  jklPiExtension(pi as unknown as ExtensionAPI)

  assert.deepEqual([...handlers.keys()], ["session_start", "agent_start", "agent_end", "session_shutdown"])
  await handlers.get("agent_start")?.({}, ctx)

  assert.deepEqual(statuses, ["jkl: working"])
})

type MockContext = {
  ui: {
    setStatus(key: string, text: string | undefined): void
  }
}
