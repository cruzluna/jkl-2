import assert from "node:assert/strict"
import test from "node:test"

import { JklPlugin, buildJklUpsertCommand, statusFromOpenCodeEvent } from "../index.js"

test("maps OpenCode session statuses to jkl pane statuses", () => {
  assert.equal(statusFromOpenCodeEvent({ type: "session.status", status: { type: "busy" } }), "working")
  assert.equal(statusFromOpenCodeEvent({ type: "session.status", status: { type: "retry" } }), "working")
  assert.equal(statusFromOpenCodeEvent({ type: "session.status", status: { type: "idle" } }), "waiting")
  assert.equal(statusFromOpenCodeEvent({ type: "session.idle" }), "waiting")
  assert.equal(statusFromOpenCodeEvent({ type: "message.updated" }), undefined)
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
  const calls = []
  const logs = []
  const plugin = await JklPlugin({
    $: async (strings, ...values) => {
      calls.push({ strings, values })
    },
    client: {
      app: {
        log: async (entry) => {
          logs.push(entry)
        },
      },
    },
  })

  await plugin.event({ event: { type: "session.status", status: { type: "busy" } } })

  assert.equal(calls.length, 1)
  assert.equal(calls[0].strings[0], "bash -lc ")
  assert.match(calls[0].values[0], /--status working/)
  assert.equal(logs[0].body.service, "opencode-jkl")
})

test("plugin ignores unrelated events", async () => {
  const calls = []
  const plugin = await JklPlugin({
    $: async (strings, ...values) => {
      calls.push({ strings, values })
    },
  })

  await plugin.event({ event: { type: "message.updated" } })

  assert.equal(calls.length, 0)
})
