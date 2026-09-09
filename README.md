# Agent Monitor

## 0.5.6 — canonical project grouping

Remote entry example: **https://monitor.example.com**. Same-machine browser: `http://127.0.0.1:43218`. The real public hostname is deployment-specific and stays in local configuration. Open the configured remote URL, sign in through the owner-only Cloudflare Access application, then use **连接与设置 → 安装 / 添加到主屏幕**. The phone version is an installable PWA, not an APK.

Canonical source repository: `https://github.com/happyivanencoding/agentmonitor`. See [repository and privacy policy](docs/REPOSITORY.md). Agent Monitor code changes are expected to be committed and pushed to `origin/main` as part of the same development task.

0.5.6 treats AgentDock transport workspaces such as `*-isolated-model-<pid>-<suffix>` as temporary execution directories rather than top-level projects. When a matching real project exists under the local `dev` root, the collector resolves the temporary workspace back to that canonical project folder; otherwise all temporary workspaces with the same transport slug still collapse to one project slug. This prevents one application from exploding into dozens of pseudo-project folders while preserving the exact temporary `cwd` in Agent evidence. Usage analytics also coalesces previously recorded isolated-workspace aliases at read time, so historical token rows do not recreate the split project list. See [0.5.6 project-grouping acceptance](docs/VERIFICATION_0_5_6.md).

0.5.5 separates Task steps and captured Request/tool records from the heavy Agent/process/token scan. Native progress refreshes independently every two seconds; web polling receives those updates instead of waiting for a full collection (about 42 seconds in the real acceptance environment). A later slow snapshot cannot restore old steps. Full metrics retain their actual sampling time, and the UI distinguishes older metrics from current steps. See [0.5.5 latency acceptance](docs/VERIFICATION_0_5_5.md).

0.5.4 fixed the observed browser capture failure and makes native plans usable without ACP. **工作总览** and **任务看板** now expose expandable local Task steps independently of browser attribution. Paired ChatGPT tabs use exact message IDs, long causal ancestry, authenticated mapping recovery, canonical connector tool paths and durable event delivery. An exact checkpoint can associate a Task with a Request, explicitly labeled as participation rather than creation; reading another Task cannot claim it. Settings show actual Request/tool ingress and capture errors instead of equating page heartbeat with successful capture. See [0.5.4 acceptance](docs/VERIFICATION_0_5_4.md).

The older 0.5.2 policy remains: every user message that actually triggers execution must create its **own** AgentDock Task/Steps, even when it is a short continuation such as “测试一下” or “再看看”. The browser observer can also re-read the exact current ChatGPT conversation mapping to backfill `task_manage` and ordinary AgentDock tool events for the current user-message ID, covering interruption/alternate transport paths where the live stream itself does not expose those frames. While a Request is still responding and no plan has arrived yet, the UI says **等待执行计划**; observed execution without a plan says **未认领计划**; **无执行计划** is reserved for completed requests that truly did not execute anything.

0.5.1 fixes the first live Chrome Request-capture blocker. The unpacked extension had referenced `extractor.cjs` as a manifest content script; current Chromium/Chrome did not inject that content-script group. The browser runtime now uses `extractor.js`, while Node tests load that same source through a test helper. The settings page also distinguishes **extension background heartbeat** from a genuinely live **ChatGPT page observer**, so a connected background worker no longer falsely implies Request capture is working.

0.5.0 makes the primary monitoring hierarchy **Project → ChatGPT Conversation → User Request → Steps → Execution**. Every observed ChatGPT user message becomes a Request, including queries that do not launch an Agent. Execution requests can claim AgentDock `task_manage` steps; ordinary AgentDock tools are recorded even when no ACP/Codex Agent is launched. If the plan changes during execution, supplemental AgentDock Tasks created in the same Request are merged as live plan revisions, so new repair/test steps appear immediately instead of leaving the original plan stale.

The 0.4.1 timeline behavior remains: the global timeline is visible in **工作总览**, defaults to **1h**, and places the newest time on the left with older time extending to the right.

The Windows desktop, browser and phone share one real collector and the same task/conversation bindings. The browser is fully interactive for monitoring, search, timelines and explicit metadata edits. It does not expose shell commands, Agent termination/restart, native settings or the browser-extension pairing key. Source files stay on the PC; monitor metadata is transmitted through the protected Cloudflare connection.

Attention items can be **归档** from the overview row or Agent detail. This is a Monitor-owned acknowledgement only: the observed `FAILED` / `SUSPECTED_STALLED` evidence is not rewritten, the source Agent is not stopped or restarted, and archived work remains visible under **全部记录** with a restore action. If that exact thread later produces activity newer than the archive action, it automatically returns to the current/attention views.

ChatGPT web attribution is a first-class setup flow in the Windows app. After the bundled Chrome/Edge extension is loaded and paired once, its popup does not need to stay open: each tab reports its exact `conversation_id`, each user-message ID, and structured AgentDock invocation/result metadata. Direct `read_file`, `file_edit`, `exec_command`, `task_manage` and other AgentDock tools can therefore belong to a Request without ACP. ACP results still require exact ACP/thread identity before Monitor joins them to Codex. The extension does not copy full transcripts, tool stdout or arbitrary command bodies into Monitor storage.

The **任务看板** exposes both unassigned native plans and attributed Request progress. Native plans do not require an ACP Agent or a browser binding. An execution Request may reference one initial AgentDock Task plus zero or more supplemental Tasks created when the model discovers necessary extra work. Monitor merges all of those plan segments and shows completed/total steps, the current step and revision count. The older AgentDock/Monitor Task timeline remains below it for history and uncaptured legacy work.

The primary work view uses **project folder → ChatGPT conversation → Request → Steps / Execution**. The Agent directory and global execution timeline still preserve their lower-level **project → conversation / Task → Agent** hierarchy for ACP/Codex evidence. Project sections can be collapsed as a whole; conversation/task groups are collapsed by default and expand to their individual ACP/Codex agents only when needed. This keeps large histories readable without losing the exact underlying agents.

The desktop collector also runs a 24-hour stale cleanup. If a logical Agent has had no activity for more than one day and still has an open AgentDock ACP session, Monitor invokes AgentDock's own `acp_session close` action and records the result. It never kills a shared Codex host PID. Codex-only stale records have no safe per-thread kill primitive, so they simply expire from current/attention/timeline views while remaining available in **全部记录**.

The PC must remain on, awake, connected and running Monitor. A stale/failed connection is shown as OFFLINE rather than silently reusing old state. No API snapshots or authentication data are stored in the PWA cache. See [remote access, login and deployment](docs/REMOTE_ACCESS.md) for setup and boundaries.


A real, local-first Windows desktop monitor for AI work, built with **Tauri 2, Rust, React, TypeScript and SQLite**. The unit of work is a logical Agent/thread, not a PID. The installed application reads real AgentDock and Codex data without a Python or Node runtime.

## Core monitoring

The shared collector connects ACP `remote_session_id` to the exact Codex thread and reads model, provider, reasoning effort, cumulative token usage, turn lifecycle and tool activity. Structured JSONL fills gaps where ACP turns are absent from Codex's turn-history projection. Windows lock-owner queries associate logical threads with shared host processes. Conversation grouping uses explicit evidence; unassigned work stays visibly unassigned.

Six pages provide work overview, timelines, process inventory, observed token usage, a project/time task board, and connection/setup controls. The app includes light/dark themes, a tray menu, optional current-user startup, explicit fallback bindings and a local observation journal.

**Important limitation:** the bundled extension must still be loaded and paired once in the desktop browser because Chrome/Edge do not let a normal desktop app silently install an unpacked extension. After that one-time browser action, attribution is automatic. Phone/native ChatGPT messages and uninstrumented or unopened browser conversations cannot be observed directly by this Windows extension. Their local AgentDock Tasks can still show live steps under 本机执行计划, with unknown conversation association stated explicitly. Nothing guesses attribution from timestamps, project paths or the shared AgentDock parent directory.

## Install and use

Build output:

```text
src-tauri\target\release\bundle\nsis\Agent Monitor_0.5.6_x64-setup.exe
```

Run the per-user installer, then open **Agent Monitor** from the Windows Start menu. No business-project changes are needed. WebView2 is the desktop rendering runtime. This local installer is unsigned; it is not a claim of code-signed public distribution.

The installed app also serves the packaged web/PWA assets on loopback port 43218. The first snapshot may take several seconds. Heavy collections run after a 2.5-second interval; their actual cadence includes collection time. Task steps and captured Requests/tools use a separate 2-second refresh path. The overview initially shows current work and the last 24 hours; **全部记录** exposes older work in the monitored collection. Click an Agent to inspect its IDs, status evidence, tool timeline, host and task association.

Closing the window leaves Monitor in the system tray. Use **Quit Agent Monitor** in its tray menu to stop it. Automatic Windows login startup is controlled in **连接与设置**. It is enabled on this remote-monitoring deployment; other installations remain opt-in. The installed collector is currently left running in the tray. See [0.5.4 capture/native-plan verification](docs/VERIFICATION_0_5_4.md), [0.5 Request/dynamic-plan verification](docs/VERIFICATION_0_5.md), [0.4 task/stale-cleanup verification](docs/VERIFICATION_0_4.md), [0.3 attribution verification](docs/VERIFICATION_0_3.md), and the earlier [0.2 release acceptance](docs/VERIFICATION_0_2.md).

## Enable automatic ChatGPT attribution

Open **连接与设置 → ChatGPT 自动归属**. Choose **打开 Chrome 扩展页** or **打开 Edge 扩展页**; Monitor opens both the browser's extension manager and the exact bundled extension folder. Enable developer mode, choose **加载已解压的扩展程序**, select that folder, then copy the pairing key into the extension popup once. The extension injects already-open ChatGPT tabs after pairing or an extension update; the popup need not stay open. Each subsequently observed user message becomes a Request and structured AgentDock activity is attached to that exact Request without opening the popup again.

For ordinary AgentDock tools, Monitor records only safe invocation metadata such as tool name, action and bounded path/project hints; it does not persist tool output or arbitrary command bodies. ACP attribution remains stricter: only recognized structured ACP results with exact IDs prove a launched Agent. When ACP evidence is unavailable, use the Agent's **手动绑定（兜底）** action.

A calling workflow that already knows its actual source conversation URL can use the source helper:

```powershell
.\scripts\Register-AgentOrigin.ps1 `
  -EntityId 'acps_<actual-returned-ID>' `
  -ConversationUrl 'https://chatgpt.com/c/<actual-conversation-UUID>' `
  -Title 'Optional known title'
```

This only writes Monitor attribution. It does not launch, stop or alter an Agent. See [attribution evidence and limitations](docs/ATTRIBUTION.md).


## Global execution progress policy

0.5.2 uses user Skill **`execution-progress` 1.0.1** in `~/.agentdock/skill-store` and keeps a readable mirror at `~/.agentdock/policies/global-execution.md`. The Skill is indexed by `agentdock_context`; every user message that actually performs work must create its own `task_manage` plan, checkpoint each completed stage, and finalize it before reporting completion. A new user message must not reuse the previous Request's Task as its own plan. This does not patch the AgentDock executable or installation directory.

Current `task_manage` does not expose an add-step operation for an existing Task. When execution discovers genuinely necessary extra work, the policy therefore creates a supplemental Task titled `补充 · ...` inside the same ChatGPT Request. Agent Monitor merges those Task IDs into one dynamic plan and displays the revision count. If AgentDock later exposes native add/insert-step support, the policy should switch to that instead.

## Data and accounting

Source discovery defaults to `%USERPROFILE%\.agentdock` and `%USERPROFILE%\.codex`. Source SQLite is read-only. Monitor owns `%LOCALAPPDATA%\AgentMonitor\monitor.sqlite`; normalized observations are retained for up to 90 days. The UI's diagnostic export omits titles, IDs, paths, commands, prompts and credentials. Remote viewing is separately authorized; the public URL never points at the local attribution bridge or AgentDock command API.

Thread tokens are real cumulative `threads.tokens_used` values. **Tokens Today** is a separate lower-bound sum of positive deltas during continuous observation. Initial lifetime totals, counter resets and gaps are never counted as today's usage. This is not ChatGPT browser-model token usage, context-window occupancy, money or a subscription bill.

Timeline coverage is explicitly partial: initial JSONL tails are bounded to 1 MiB; detailed SQLite projections return at most 4,000 recent items. Missing duration is shown as an instantaneous/unknown event, not invented reasoning time. Tool interval totals remove overlap; the remaining wall-clock gap is not falsely labeled model time.

## Develop and verify

Windows build prerequisites are the Rust toolchain, Microsoft C++ Build Tools, WebView2 and a supported Node LTS installation. The source includes lockfiles.

```powershell
npm ci
npm run desktop:dev

npm run build
npm test
cargo test --manifest-path src-tauri/Cargo.toml
npm run desktop:build
```

A standalone source-data probe is available in the executable:

```powershell
.\src-tauri\target\release\agent-monitor.exe --probe .local\probe.json --twice
```

For **embedded** development verification, build with `npm run tauri -- build --debug --no-bundle`, then launch that debug executable with `--qa-native`. The debug-only harness drives the actual native frontend/IPC and writes `%LOCALAPPDATA%\AgentMonitor\native-qa.json`. It creates and removes a clearly labeled temporary binding. Screenshots/probes belong only in ignored `.local/`; do not commit them. The installed release has no QA injection or remote-debugging configuration.

See [verification](docs/VERIFICATION.md), [architecture](docs/ARCHITECTURE.md), [security](docs/SECURITY.md), and [maintenance handoff](DEEP_CONTEXT_HANDOFF.md).

Web acceptance uses `node scripts/verify-web.cjs` against the running collector. The HTTP/Cloudflare boundary is exercised with `node scripts/verify-remote-boundary.cjs`. Private reports and screenshots stay under `.local/web-qa/`. Current owner login on a physical phone must be confirmed by the owner; local-browser tests and anonymous public rejection do not impersonate that login.
