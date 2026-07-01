import assert from "node:assert/strict"
import test from "node:test"

import { JklPlugin, buildJklUpsertCommand, statusFromOpenCodeEvent } from "../index.ts"

test("maps OpenCode session statuses to jkl pane statuses", () => {
  assert.equal(
    statusFromOpenCodeEvent({ type: "session.status", properties: { status: { type: "busy" } } }),
    "working",
  )
  assert.equal(
    statusFromOpenCodeEvent({ type: "session.status", properties: { status: { type: "retry" } } }),
    "working",
  )
  assert.equal(
    statusFromOpenCodeEvent({ type: "session.status", properties: { status: { type: "idle" } } }),
    "waiting",
  )
  assert.equal(statusFromOpenCodeEvent({ type: "session.idle" }), "waiting")
  assert.equal(statusFromOpenCodeEvent({ type: "message.updated" }), undefined)
})

test("keeps compatibility with legacy flat session status events", () => {
  assert.equal(statusFromOpenCodeEvent({ type: "session.status", status: { type: "busy" } }), "working")
})

test("builds a tmux-aware jkl upsert command", () => {
  const command = buildJklUpsertCommand("working")

  assert.match(command, /\[ -n "\$TMUX" \] \|\| exit 0/)
  assert.match(command, /command -v jkl/)
  assert.match(command, /jkl upsert/)
  assert.match(command, /#\{session_id\}/)
  assert.match(command, /#\{pane_id\}/)
  assert.match(command, /--status working/)
})

test("plugin syncs busy status through the OpenCode shell helper", async () => {
  const calls: Array<{ strings: TemplateStringsArray; values: unknown[] }> = []
  const logs: unknown[] = []
  const plugin = await JklPlugin({
    $: async (strings: TemplateStringsArray, ...values: unknown[]) => {
      calls.push({ strings, values })
      return undefined
    },
    client: {
      app: {
        log: async (entry: unknown) => {
          logs.push(entry)
        },
      },
    },
  } as unknown as Parameters<typeof JklPlugin>[0])

  await plugin.event?.({ event: { type: "session.status", properties: { status: { type: "busy" } } } } as Parameters<
    NonNullable<typeof plugin.event>
  >[0])

  assert.equal(calls.length, 1)
  assert.equal(calls[0]?.strings[0], "bash -lc ")
  assert.match(String(calls[0]?.values[0]), /--status working/)
  assert.equal((logs[0] as { body: { service: string } }).body.service, "opencode-jkl")
})

test("plugin ignores unrelated events", async () => {
  const calls: Array<{ strings: TemplateStringsArray; values: unknown[] }> = []
  const plugin = await JklPlugin({
    $: async (strings: TemplateStringsArray, ...values: unknown[]) => {
      calls.push({ strings, values })
      return undefined
    },
    client: {
      app: {
        log: async () => {},
      },
    },
  } as unknown as Parameters<typeof JklPlugin>[0])

  await plugin.event?.({ event: { type: "message.updated" } } as Parameters<NonNullable<typeof plugin.event>>[0])

  assert.equal(calls.length, 0)
})
