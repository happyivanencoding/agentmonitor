# Agent Monitor 0.5.0 — Request progress and dynamic plan verification

Verified locally on 2026-09-07 against `C:\dev\agent-monitor` and the installed per-user Agent Monitor runtime. No Git operation was required for this iteration.

## Scope

0.5.0 changes the primary progress model from ACP-first monitoring to:

`Project → ChatGPT Conversation → User Request → Steps → Execution`

A Request is one observed ChatGPT user message. It may have no execution, direct AgentDock tool execution, one or more AgentDock `task_manage` plans, ACP/Codex Agents, or a mixture of those.

Plan changes are dynamic. The current AgentDock `task_manage` public schema does not expose add/insert-step for an existing Task, so a genuinely new required work segment is represented by a supplemental Task created in the same Request. Agent Monitor merges all observed task IDs for that Request and reports one combined progress view plus `revisionCount`.

## Build and deterministic tests

The checks below were chosen because a failure would change the implementation: parser tests catch incorrect ChatGPT/tool attribution, Rust tests catch persistence/status regressions, the frontend build catches Request UI/schema mismatches, and the Tauri build confirms the installed bundle includes the extension and progress Skill source.

- `npm test`: **25 / 25 passed**.
  - exact ACP attribution remains strict;
  - direct AgentDock invocations are recognized as Request tool activity without ACP;
  - `task_manage` results expose task IDs to the same Request;
  - tool stdout and arbitrary command bodies are not copied into request activity metadata;
  - ordinary `read_file` output containing ACP-looking JSON cannot become an ACP binding.
- `cargo test --manifest-path src-tauri/Cargo.toml`: **35 / 35 passed**.
  - every observed user-message ID deduplicates to one Request;
  - Request tool events preserve initial and supplemental task IDs;
  - existing archive, token, ACP, remote-access and stale-state tests still pass.
- `npm run build`: passed.
- `npm run desktop:build`: passed and produced:

```text
src-tauri\target\release\bundle\nsis\Agent Monitor_0.5.0_x64-setup.exe
```

## Durable AgentDock progress policy

A first-party user Skill was created, linted, validated, installed and activated:

```text
execution-progress 1.0.0
```

`skill-authoring` lint returned `portable=true`, zero errors and zero warnings. `skill_package validate` passed, and `skill_package install` activated the Skill under the user AgentDock skill store. A subsequent real `agentdock_context` call listed:

```text
execution-progress
```

with the execution-wide trigger description. A readable local mirror exists at:

```text
%USERPROFILE%\.agentdock\policies\global-execution.md
```

The policy is not patched into `AppData\Local\AgentDock\bin`. Agent Monitor 0.5.0 also bundles a copy of the Skill source in its own package and reports the currently active user-Skill state through `sources.progressPolicy`.

## Installed runtime

The 0.5.0 NSIS per-user upgrade exited with code 0. The installed executable reports file/product version `0.5.0`. The installed app was launched detached and remained running.

The local bridge health endpoint returned:

```json
{"app":"Agent Monitor","version":"0.5.0"}
```

The installed snapshot API reported:

- `version = 0.5.0`;
- `sources.progressPolicy.healthy = true`;
- `sources.progressPolicy.activeVersion = 1.0.0`.

## Local end-to-end Request checks

Two authenticated loopback bridge checks exercised the installed 0.5.0 request endpoints using the same bounded payload contract used by the browser extension. They were synthetic integration records only and were deleted after the checks; the app was restarted afterwards so no fake extension heartbeat remained.

### Direct AgentDock / Task Request

A synthetic Request was associated with the real current AgentDock development Task through a normal `AgentDock.task_manage` tool event. After the collector refreshed, the installed snapshot showed:

- project `agent-monitor`;
- `progressProtocol = claimed`;
- `5 / 7` steps from the real AgentDock Task;
- one direct AgentDock tool event;
- no ACP requirement for creating the Request/tool relationship.

### Dynamic plan revision

A second synthetic Request was associated with both the main development Task and the real supplemental Task created after the owner added the dynamic-plan requirement. The installed snapshot showed:

- `plans = 2`;
- `revisionCount = 1`;
- combined progress `7 / 10`;
- current step resolved to the supplemental plan's active validation step;
- project `agent-monitor`;
- `progressProtocol = claimed`.

This verifies that newly discovered work can expand the visible remaining plan without editing AgentDock Task JSON.

After cleanup and restart, the installed snapshot again reported zero synthetic Requests and no synthetic `lastExtensionSeen` state.

## Browser acceptance still required

Immediately after synthetic-row cleanup/restart, `sources.bridge.lastExtensionSeen` was null. A later current-runtime check then observed the paired Chrome extension heartbeat from `chrome-extension://<extension-id>`, and the installed extension payload on disk reports version `0.5.0`.

This is useful pairing/connection evidence, but **it is still not proof that a real ChatGPT turn has executed the newly loaded 0.5.0 page observer**: an unpacked extension can retain older loaded code until Chrome reloads it.

Because this is an unpacked browser extension, the owner should reload/update the Agent Monitor extension once and refresh an open ChatGPT tab. The current heartbeat suggests pairing is already present; pair again only if the popup reports otherwise. The next real user message should then be used to confirm live:

`conversation → user message Request → direct AgentDock/task_manage activity → optional ACP`

The deterministic parser, installed bridge, persistence, collector, dynamic-plan merge and UI bundle are verified; the current private ChatGPT response shape has not yet been accepted through the owner's authenticated browser for 0.5.0.
