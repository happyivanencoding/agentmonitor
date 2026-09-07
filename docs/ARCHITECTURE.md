# Architecture and invariant decisions

One Tauri process hosts a Rust collector, SQLite observation/task journal, authenticated loopback attribution bridge, the 24-hour stale ACP closer, and a React UI. No Python/Node runtime is required by the installed app.

## Ownership graph

Project -> ChatGPT conversation -> exact user-message Request -> declared Steps -> Execution. Execution can be direct AgentDock tools, one or more AgentDock `task_manage` Tasks, ACP/Codex Agents, or a mixture. The ACP branch remains explicit ACP identity -> `remote_session_id` -> Codex thread -> turn -> tool; `thread_spawn_edges` provides logical subagent edges. OS processes are a separate layer. The parent ACP directory is **not** a conversation ID: all 365 inspected ACP records shared one parent. No time/cwd-based cross-conversation attribution is allowed.

## Sources

Source SQLite is opened read-only with short busy timeouts, using live WAL-aware reads, never `immutable=1`. Column discovery handles schema drift. ACP JSON is cached with modification metadata. Rollout JSONL is incrementally tailed for structured lifecycle/tool/token metadata where the turn-history projection is absent. A bounded initial tail is explicitly reported as partial. Full ChatGPT transcripts, reasoning content, tool outputs, diffs, auth files and browser cookies are not persisted by Monitor. Monitor-owned SQLite stores `chat_requests` keyed by exact conversation + user-message ID and bounded `request_tools` metadata. It may retain the user's Request text so the owner can identify the task, but ordinary tool stdout and arbitrary command/file bodies are excluded. ACP bindings remain separately evidenced and strict.

Windows Restart Manager **queries only** establish thread-writer-lock ownership. This was verified against seven logical threads sharing one Codex host. `commandExecution.processId` is a Codex tool-session handle, not assumed to be an OS PID.

The only automatic control action is stale ACP cleanup. At >24 hours without logical-Agent activity, an open AgentDock ACP session is closed through AgentDock's own local MCP `acp_session close`. A Codex-only stale thread is never implemented as a shared-PID kill; it simply expires from live views because a host may own multiple logical threads.

## Status

Terminal turn evidence wins over process heuristics. Closed ACP transport does not imply success. CPU inactivity never means death. Stalls require an active uncompleted turn, stale structured activity, repeated unchanged observations and no positively observed child activity. Missing evidence means UNKNOWN, not fabricated health.

## Tokens

Thread totals come directly from `threads.tokens_used`. They are not context-window size, ChatGPT browser usage, or a monetary bill. A new observation starts with a baseline, never adds lifetime tokens to today. Only continuously observed positive deltas are counted; restarts/gaps/counter resets rebaseline. Aggregation deduplicates logical threads, not ACP aliases.

## Security

Loopback only; random bearer token; bounded JSON bodies; exact Host validation; extension-origin CORS only; no arbitrary filesystem or command endpoints. External URL opening is restricted to HTTPS ChatGPT conversation URLs. Source observation remains read-only; stale ACP closure is a separate narrow AgentDock control path. Secrets/runtime databases never enter Git.

## 0.2 web/mobile extension

The process diagram is now:

```
AgentDock + Codex read-only stores
        -> one Rust collector -> one in-memory snapshot + own SQLite journal
                |-> Tauri IPC -> Windows window/tray
                |-> HTTP 127.0.0.1:43218 -> same React UI / PWA
                                         <- Cloudflare Tunnel + Access
```

`api.rs` contains shared operations used by both transports. `remote.rs` serves only packaged UI assets and the explicit Monitor API; four worker readers are sufficient for this single-user workload. `access.rs` validates the configured application's Cloudflare JWT with cached public signing keys. Missing remote configuration permits only the actual loopback browser path; invalid configuration stops the web listener and records an error, without granting public access.

`src/transport.ts` selects Tauri IPC or same-origin HTTP at runtime. Browser polls do not overlap, pause when hidden and reconnect on visibility/network changes. Conditional delivery uses existing collection/attempt timestamps, with gzip for substantial bodies. This avoids duplicate collectors, speculative event infrastructure and unnecessary content hashes. Native command/event behavior remains native.

`mobile.css` adapts the existing hierarchy for narrow screens; `RemoteSettings.tsx` provides installation, identity and connection controls instead of exposing native-only settings. The PWA manifest and offline worker are packaged alongside the desktop assets. Mobile is an installation mode of the web app, not a second data model.

Public access does not repair missing launch attribution. The original extension and explicit-evidence rules remain authoritative. See `REMOTE_ACCESS.md` for route protection, operations, startup and first-login limitations.
