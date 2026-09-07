# Local investigation — 2026-09-06

Read-only checks through AgentDock confirmed:

- AgentDock 0.8.1; runtime actual port **8766**, discovered from runtime.json.
- Multiple ACP JSON records shared one parent directory. Parent identity cannot be treated as per-conversation attribution.
- The Codex state database contained substantial logical-thread history; model/provider/reasoning/tokens are directly available from structured stores.
- The turn-history database has thread_turns and thread_items, including commandExecution/fileChange/mcpToolCall/reasoning/collabAgentToolCall. Historical inProgress rows existed, so this label alone cannot mean currently running.
- A sampled ACP `remote_session_id` matched a Codex thread that had **no** turn-history projection, while its rollout still contained terminal lifecycle and matching cumulative token evidence.
- Restart Manager identified multiple distinct thread-writer locks held by one live Codex Desktop host. A PID can host multiple agents.
- ACP bundled executable and desktop Codex executable have different paths.
- Installed AgentDock log only exposes tool name/duration, without a conversation/request attribution identifier in the inspected records. The stated AgentDock working directory is not a source Git checkout.
- The existing Chrome instance was not launched with remote debugging; no browser profiles, login cookies or account credentials were extracted.

These are observations of the local installation, not a promise that undocumented schemas will never change.
