# Conversation attribution

## What is proven locally

`ACP JSON.remote_session_id` is the exact Codex logical thread ID. Model, provider, reasoning effort and cumulative tokens are read from Codex structured stores. Seven independently named thread-writer locks were observed on one host process; PID is not an Agent identity.

The parent directory of ACP session JSON is **not** a ChatGPT conversation. All 365 inspected records shared one parent namespace. Binding that directory would incorrectly merge unrelated browser conversations and business-app calls.

## Evidence levels

| Source | Meaning |
|---|---|
| `browser-tool-result` | A paired extension observed a recognized structured ACP invocation result, carrying an exact ACP ID and remote thread ID, within an identified conversation. This remains the strict Agent-launch evidence. |
| Request/tool observation | A paired extension observed an exact conversation + user-message Request and a structured `AgentDock.*` invocation/result in that same turn. This proves Request execution activity, but ordinary tools do not prove an ACP Agent was launched. |
| Task create result | An exact create result in this Request returned the Task ID; plan relation is `created`. |
| Task checkpoint invocation | An exact `task_manage checkpoint` in this Request names the Task ID; plan relation is `checkpointed`, not a claim of creation. Read/list alone is insufficient. |
| `user-confirmed-desktop` | A person explicitly associated an existing ACP/thread with a ChatGPT conversation URL in the desktop UI. |
| `user-confirmed-browser-reference` | Explicit registration from the extension popup or local registration helper. |
| Inherited thread binding | A child inherits its parent's evidence through a real Codex spawn edge; the UI retains the origin thread. |
| Unknown | No suitable evidence. No time, workspace, title similarity, CPU or shared-parent heuristic is used. |

A deterministic ID match is not cryptographic attestation by ChatGPT. Browser evidence depends on the integrity of the paired browser and its page. It authorizes no process control, code execution, credential access or business-project mutation.

## Extension setup

Open the desktop app's **连接与设置 → ChatGPT 自动归属** page. Choose the Chrome or Edge setup button; Monitor opens both that browser's extension manager and the exact bundled extension directory. Enable developer mode, load that unpacked directory, copy the pairing key into the extension popup once, pair. Current-version scripts are injected into already-open ChatGPT tabs without refreshing them. This is a per-browser, user-controlled one-time installation; the installer does not silently alter Chrome/Edge profiles or enterprise policies.

After pairing, the popup does not need to remain open. The extension heartbeats when the browser starts and when its ChatGPT observer becomes ready. Each tab keeps its own `conversation_id`; each observed user-message ID becomes one Request. Structured `AgentDock.*` invocations/results in that turn are attached to the same Request. If the tool is ACP, the recognized launch result additionally emits the exact conversation + exact ACP pair and remote thread identity. Multiple ChatGPT tabs and multiple Requests inside one conversation therefore remain distinct without relying on ordering or timestamps.

The extension listens only on `https://chatgpt.com/`. MAIN-world code observes the page's existing conversation fetch responses, while an isolated relay sends bounded Request/tool attribution metadata to the background worker. Browser manifest content-script resources use `.js`; Node tests load the same `extractor.js` source through a CommonJS test helper. This matters because the previous `.cjs` manifest resource was not injected by current Chrome/Chromium. The worker alone holds the local pairing key and contacts `http://127.0.0.1:43217`. The worker does not read cookies or receive ChatGPT credentials. MAIN-world code reads only the same-origin signed-in session when needed for the exact conversation mapping; its access token remains in the page closure and is never persisted or relayed. No arbitrary transcript or ordinary tool output is copied to Monitor. Monitor may store the current user Request text so the owner can recognize the task, but it does not persist ordinary tool stdout, arbitrary command bodies or file replacement content.

## Strict automatic parser

Request/tool activity requires an identified conversation, an exact user-message ID and a structured AgentDock invocation/result associated with that turn. Direct `AgentDock.*` recipients and generic `api_tool` wrappers are supported, including `/AgentDock/link_<opaque>/task_manage` paths, which are canonicalized before interpreting Task/ACP semantics. Only bounded safe invocation metadata is emitted.

ACP extraction is intentionally stricter than ordinary Request execution. It requires a tool-authored recognized ACP creation/invocation result containing both `acps_...` and `remote_session_id` (or the explicit supported telemetry marker). A generic wrapper additionally requires the corresponding structured assistant invocation. Full conversation mappings use actual parent links, not nearby timestamps. A conflicting conversation ID aborts extraction. Ordinary `read_file`/directory output containing ACP-looking fields cannot establish Agent launch evidence.

## Requests, Tasks and dynamic plans

Every observed ChatGPT user message becomes one Request, including a pure query that launches no tool. A Request is therefore not the same object as an AgentDock Task. When execution occurs, `execution-progress` 1.0.1 requires a **new Task for that exact Request**, even when the user message is only a continuation of earlier work; a previous Request's Task must not be resumed as the current Request's plan. Direct AgentDock tools and optional ACP/Codex Agents remain execution evidence under it.

Current AgentDock 0.8.1 does not expose add/insert-step for an existing Task. If the model discovers genuinely necessary extra work, the active `execution-progress` Skill instructs it to checkpoint the original plan and create a supplemental `补充 · ...` Task in the same Request. Monitor merges Task IDs proven by creation results or explicit checkpoint participation in that Request and shows the combined step count, current step and plan revision count. It never edits AgentDock Task JSON directly.

Legacy automatic Monitor Tasks from 0.4 remain readable for older ACP-based history, but the primary 0.5 owner-facing unit is Request progress.

## Capture reliability and acceptance

0.5.4 observes exact message IDs rather than prompt text, so repeated continuations and attachment-only requests remain distinct. Long parent chains are traversed with cycle detection instead of a 60-message cutoff. An assistant `recipient="all"` message completes a Request only when its explicit end-turn marker is present; commentary and unfinished streams do not imply completion. Mapping recovery authenticates in the page before reading the exact conversation, including deployments where unauthenticated reads return 404 instead of 401. Known alternate envelopes are supported; unknown encodings fail visibly.

The isolated relay acknowledges only after the worker has persisted sanitized events. Temporary Monitor outages therefore queue events for retry rather than permanently marking them sent. Replayed starts cannot reopen completed tools. Manifest-version metadata rejects stale observers that survive unpacked-extension updates. Current status separates observer liveness, mapping failures, last Request ingress and last tool ingress.

The extension-owned `apply-update.html` page can deliberately apply copied extension files without refreshing ChatGPT tabs. Obtain its extension ID from the paired bridge or browser's own extension entry; do not hard-code a personal deployment ID in source. First installation still requires the browser's normal user-controlled extension setup.

Browser collection is not account-wide synchronization. It covers matching, instrumented tabs and their exact current messages. No extension can infer text that was never exposed to it. Local Task collection is separate: **本机执行计划** renders actual persisted steps even when the owner used a phone/native client or no ACP was launched. A Task without browser evidence remains unassigned, not a fake conversation.

The release has 44 Node tests and 36 Rust tests. `scripts/verify-capture.cjs` loads a real Chromium extension with explicitly synthetic transport; `scripts/verify-native-plans.cjs` checks the installed collector/UI against a real Task. The real signed-in acceptance also recovered seven tool records and an exact checkpoint association showing 3/3 steps. See [0.5.4 verification](VERIFICATION_0_5_4.md). Real prompts, IDs and private browser/account data are not committed as fixtures.

## Explicit local registration

For workflows that already know the source conversation URL, the source repository includes `scripts/Register-AgentOrigin.ps1`:

```powershell
.\scripts\Register-AgentOrigin.ps1 `
  -EntityId 'acps_<exact ID returned by AgentDock>' `
  -ConversationUrl 'https://chatgpt.com/c/<actual conversation UUID>' `
  -Title 'Optional known conversation title'
```

Call it immediately after creating/resuming the relevant ACP session. It reads the Monitor's local pairing key without printing it and uses the authenticated loopback API. It does not start, stop, restart or alter an Agent. The Monitor must already be running. No changes to business projects or the installed AgentDock binary are required.

Do not invent the conversation UUID. The extension popup can also explicitly bind the current conversation to an exact ACP ID when automatic extraction is unavailable.

## Conflict handling and aliases

A binding cannot silently move to another conversation: the server returns a conflict. Remove the old evidence explicitly in the desktop app before changing its origin. Conflicting ACP aliases for the same logical thread yield an attribution conflict rather than an arbitrary winner. Thread token totals are counted once regardless of ACP aliases.

## Deliberate limits

A Windows extension cannot observe ChatGPT pages on a phone, another computer or an uninstrumented browser. Historical records without captured source identifiers cannot be reliably reconstructed from timestamps. Such work remains unassigned until explicit evidence is provided. ChatGPT's own browser-model token usage is separate from the local Codex thread usage and is not available through these stores.

Source modifications to AgentDock were not made. If a future AgentDock version exposes invocation-scoped caller/conversation metadata, add a dedicated adapter and preserve the exact evidence source instead of replacing browser/manual bindings heuristically.
