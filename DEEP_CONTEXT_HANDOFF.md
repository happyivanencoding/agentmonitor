# Agent Monitor — maintenance handoff

Updated: 2026-09-07. Project root: `C:\dev\agent-monitor`. This is an independent project. No unrelated application project code was changed. AgentDock binaries/source were not modified; one user-level AgentDock Skill was installed under `~/.agentdock`.


## Repository workflow

Canonical remote: `https://github.com/happyivanencoding/agentmonitor`.

After every future Agent Monitor code change, update relevant docs/handoff, commit the completed change to local `main`, and push it to `origin/main` in the same task. Do not leave completed code changes only on the workstation. Before every push, keep the public tree free of personal names/emails, user-specific profile paths, credentials, pairing keys, runtime databases, deployment IDs, screenshots, real prompts/transcripts and unrelated project data. Deployment-specific values belong in ignored runtime files/environment variables. See `docs/REPOSITORY.md`.

## Installed and running: 0.5.2

The per-user installation is now **0.5.2**. This patch fixes the second live Request-progress gap: every user message that actually causes execution must have its own new `task_manage` Task/Steps, rather than resuming a Task created for a previous Request. `execution-progress` is now version **1.0.1**. The browser observer also re-reads the exact current ChatGPT conversation mapping for the current user-message ID so `task_manage` and ordinary AgentDock tool events can be backfilled when interruption/alternate response transports do not expose those frames in the live stream. Active Requests with no plan yet show `等待执行计划`; observed execution without a plan shows `未认领计划`; only completed no-execution Requests show `无执行计划`.

0.5.1 fixed the preceding Chrome content-script injection failure discovered during the first real Request acceptance. `extractor.cjs` was valid Node test code but was not injected when referenced as a Chrome manifest content script; the browser runtime now uses `extractor.js`, and Node tests execute that same source through `tests/load-extractor.cjs`. A real Chromium extension probe against `https://chatgpt.com/` returns `AgentMonitorParser=true` and `observer-ready`.

The primary monitoring graph remains **Project → ChatGPT Conversation → User Request → Steps → Execution**. Every user message observed by the paired browser extension becomes one Request. A Request may be a no-execution query, direct AgentDock tool work, an AgentDock `task_manage` plan, ACP/Codex execution, or a mixture. Ordinary AgentDock tools therefore remain visible even when no ACP/Codex Agent is launched.

0.5.1 also separates browser-extension background connectivity from page-observer liveness. The page observer reports every 30 seconds through the paired bridge; the desktop UI only shows “页面捕捉已连接” when that observer was seen recently. A background heartbeat alone is shown as “扩展后台已连接 · 页面捕捉未连接”.

Execution progress is declared through the user Skill **`execution-progress` 1.0.1**, installed and active under `~/.agentdock/skill-store`; `agentdock_context` lists it. A readable mirror lives at `~/.agentdock/policies/global-execution.md`. The Skill instructs execution requests to create a result-oriented `task_manage` plan, checkpoint every completed stage, block/resume honestly, and finish with final review/complete. Agent Monitor reports this Skill's active state as `sources.progressPolicy` and shows an `未认领计划` warning if execution is observed without a claimed Task plan.

Plans are deliberately dynamic. Current AgentDock 0.8.1 `task_manage` cannot append steps to an existing Task, so newly discovered necessary work is represented by a **supplemental Task** (`补充 · ...`) created inside the same ChatGPT Request. Monitor gathers all task IDs observed in that Request, merges their steps, and displays the current step, combined completed/total count and `revisionCount`. It does not directly edit AgentDock Task JSON. If AgentDock later adds native add/insert-step support, use that instead.

0.5.0 retains the 0.4.1 timeline/readability changes: larger desktop center typography, global timeline directly in **工作总览**, default **1h**, newest time on the left and older time to the right. The 24-hour stale ACP closer and remote web/PWA behavior are otherwise unchanged.

0.4.0 adds three connected product features. First, the browser attribution path now captures only the causal user turn that actually triggered a recognized ACP invocation and uses it to create/reuse a **Monitor Task**; a conversation is not a Task, and multiple ACP sessions may belong to one user goal. Second, **任务看板** merges these Monitor Tasks with AgentDock native Tasks and groups them by project and creation time on a 7/30/90-day timeline. Third, a background 24-hour stale cleanup calls AgentDock's real `acp_session close` for ACP-backed agents with no activity for more than one day, while Codex-only stale records are expired from live views without killing a shared host PID.

The work overview / **全部记录** and global timeline are no longer flat Agent lists. They now share a collapsible **project folder → ChatGPT conversation / Task → Agent** hierarchy. Conversation/task groups default to collapsed and display their Agent count; project sections can be collapsed as a whole. The grouped timeline renders a summary span for each conversation/task and reveals the exact per-Agent ACP/turn/tool bars only on expansion.

Live stale-cleanup acceptance passed against the local AgentDock store: stale ACP sessions older than 24 hours were closed successfully with zero observed failures, and a direct follow-up scan found no remaining open sessions beyond the threshold. Exact local session identifiers, timestamps and counts are intentionally omitted from the public handoff. The task/timeline code also excludes archived or >24h-inactive rows from the main timeline, so historical stale bands remain available only through historical/all-record views rather than appearing active.

0.3.0 makes **ChatGPT automatic attribution** a first-class product flow. After the bundled extension is loaded and paired once in the desktop browser, no popup needs to stay open: each ChatGPT tab reports its own exact conversation ID together with the exact ACP ID returned by that conversation, then Monitor joins ACP `remote_session_id` to the exact Codex logical thread. Concurrent ChatGPT tabs and one conversation launching multiple ACP sessions have explicit parser tests. Unknown response shapes still fail closed and remain unassigned; manual binding is now labeled as a fallback rather than the normal workflow.

The one-time browser action is still genuinely required. A normal desktop installer cannot silently install an unpacked Chrome/Edge extension into the user's profile. 0.3.0 therefore adds **连接与设置 → ChatGPT 自动归属** with Chrome/Edge setup buttons that open both the browser extension manager and the exact bundled extension directory, followed by one-time local pairing. At the final install check `lastExtensionSeen` was still null, so do not claim the user's authenticated browser is already paired or that live ChatGPT automatic capture has passed yet.

0.2.1 adds a Monitor-owned **archive / restore** disposition for attention items. Archive means “owner reviewed; no intervention needed”: it does not rewrite the observed Codex/ACP status and does not stop/retry/restart an Agent. Archived rows leave current/attention counts and remain visible under **全部记录**; if the same logical thread later reports activity newer than the archive timestamp, the archive automatically ceases to apply and the work reappears. The eight attention records explicitly selected by the owner on 2026-09-06 were archived after installation; a live snapshot then reported `stalled=0`, `failed=0`, with all eight records carrying `archived=true` while preserving their underlying statuses/evidence.

Release evidence is split by version: web/mobile boundary in `docs/VERIFICATION_0_2.md`, attribution in `docs/VERIFICATION_0_3.md`, stale cleanup/task grouping in `docs/VERIFICATION_0_4.md`, Request/dynamic plans in `docs/VERIFICATION_0_5.md`, Chrome injection in `docs/VERIFICATION_0_5_1.md`, and per-Request plan/tool backfill in `docs/VERIFICATION_0_5_2.md`. Public URL examples remain deployment-specific. A real authenticated Chrome Request was accepted under 0.5.1; after installing 0.5.2 the unpacked extension requires one Reload + ChatGPT refresh before the new tool/Task backfill path can be claimed live.

## Current release: 0.5.2 per-Request plans / tool backfill / reliable Request injection / stale cleanup / remote web / installable phone PWA

This supersedes the local-only deployment description in the historical 0.1 notes below. Public URL: `https://monitor.example.com`; local browser URL: `http://127.0.0.1:43218`. Windows, web and phone share one collector/snapshot/journal. The phone version is a PWA, not an APK; no background push or remote Agent kill/restart is implemented.

New modules: Rust `access.rs` (Cloudflare JWT), `api.rs` (common native/web operations), `remote.rs` (restricted loopback web service). Frontend `transport.ts`, `RemoteSettings.tsx`, `mobile.css` and `public/` add responsive web/PWA behavior, explicit offline/expired-login handling and static-only caching. Native-only pairing keys, filesystem openers and Windows startup controls are not remotely exposed.

A shared Cloudflare Tunnel may carry one dedicated Agent Monitor ingress while unrelated routes and policies remain unchanged. Monitor uses its own Access app/audience and an owner email supplied through local environment configuration. Private runtime `remote.json` contains the origin/team/audience. API credentials are read only by the deployment helper and are not runtime dependencies. No bypass policy is required. Authenticated public-owner login/physical-phone installation remain separate acceptance steps, not claims derived from mock authentication.

Current 0.5.2 validation: Node/browser attribution tests are **28 / 28 passed**; `npm run build` and optimized NSIS packaging pass. The installed per-user executable and loopback bridge report **0.5.2**. The preceding 0.5.1 live acceptance already proved a real authenticated Chrome user message can enter `chat_requests` with `extensionVersion=0.5.1` and `pageObserver=observer-ready`; the actual prompt text is intentionally omitted from the public handoff. 0.5.2 additionally adds exact current-conversation mapping backfill for `task_manage`/ordinary AgentDock tools and the stricter per-Request Task policy; the owner's running unpacked extension must be Reloaded once after the 0.5.2 install before that new backfill path can be accepted live.

Historical 0.5.0 validation passed the Rust/Node suites, production frontend build and optimized NSIS package. Installed-loopback integration verified direct `task_manage` Request linkage without ACP plus a dynamic Request spanning an initial and supplemental plan. Exact local history counts and cleanup volumes are intentionally omitted from the public handoff; detailed historical behavior remains in `docs/VERIFICATION_0_5.md` and `docs/VERIFICATION_0_4.md`.

One actual mobile issue was fixed: the binding dialog originally sat behind the full-screen details panel. The dialog now has an explicit higher stacking level and the mobile click/save test passes. Screenshots disable finite animations to capture stable frames. Measured ~973 KB snapshots compressed to ~152 KB; unchanged snapshots return 204 using observation timestamps, not hashing. This is still observation-based monitoring, not a cloud history archive.

Read `docs/REMOTE_ACCESS.md` before changing deployment. Never route the domain to 43217 or AgentDock's command API. Update bundled web assets and desktop package together. Credentials, captures and real-data reports stay out of Git.

## Product responsibility

Monitor **AI work**, not a count of processes. The primary graph is now **Project → ChatGPT conversation → user Request → declared Steps → Execution**. Execution can be direct AgentDock tools, one or more AgentDock Tasks, ACP/Codex Agents, or a mixture. ACP session → Codex logical thread → turn/tool remains the detailed Agent branch. PID, CPU, RAM and descendants are a separately evidenced host layer. Do not make ACP or PID mandatory for representing a user Request.

## Delivered implementation

Tauri 2/Rust hosts a mostly read-only collector, own SQLite observation/task/request journal, authenticated loopback browser bridge, the narrow owner-requested stale-session closer, and React/TypeScript frontend. Monitor-owned `chat_requests` and `request_tools` persist exact conversation/user-message Request identity plus bounded AgentDock execution metadata. AgentDock source SQLite/JSON observation stays read-only; `task_manage` progress is written by the executing LLM through AgentDock itself, not by Monitor editing Task files. The only Monitor-initiated automatic control mutation remains AgentDock `acp_session close` after 24h inactivity. The installed executable does not need Python or Node.

Real local validation loaded a substantial Codex/ACP history and multiple live logical work items. A sampled completed rollout also verified the fallback path where lifecycle evidence exists despite an absent turn-history projection. Exact local row counts, token totals and session identifiers are intentionally omitted from the public handoff.

35 Rust tests and 25 Node/browser attribution tests pass in current 0.5.0 source. The 0.5.0 frontend production build and x64 NSIS packaging pass, and the per-user installed runtime reports 0.5.0. The Request/task bridge and dynamic-plan collector behavior were also exercised against that installed runtime with temporary synthetic Request rows that were removed afterwards. See `docs/VERIFICATION_0_5.md` for current evidence; earlier release evidence remains in `docs/VERIFICATION.md` through `docs/VERIFICATION_0_4.md`.

## Crucial findings — do not rediscover the wrong architecture

All inspected ACP session files share one AgentDock parent namespace. **It is not a conversation ID.** `remote_session_id` is the exact Codex thread identity. One host process can own many thread-writer locks. `commandExecution.processId` is a tool session handle, not assumed to be an OS PID. Installed AgentDock's actual health port was 8766, from runtime.json, not the default 8765.

Some ACP turns are present only in rollout JSONL; the SQLite turn projection is incomplete. There are stale `inProgress` rows and genuine orphaned ACP records. Keep partial/unknown state visible. Never treat closed transport as success, CPU inactivity as death, or a first old open-tool observation as currently running. The latter warmup case has a regression test.

## Code map

`src-tauri/src/sources.rs` discovers source versions, opens source SQLite read-only and projects safe item metadata. `rollout.rs` tails structured JSONL. `os.rs` handles sysinfo and Restart Manager owner queries. `model.rs` defines normalized Agent states. `collector.rs` joins identities and now emits agents/native Tasks/legacy Monitor Tasks plus Request plans/progress. `storage.rs` owns conversation bindings, `chat_requests`, `request_tools`, legacy Monitor Tasks, archive, cleanup-attempt and delta accounting. `bridge.rs` exposes bounded `/v1/request`, `/v1/tool`, binding and heartbeat endpoints to the paired extension. `extension/` observes exact ChatGPT user-message/tool parentage; ordinary AgentDock tools produce bounded Request execution evidence while ACP binding remains strict. `agentdock-skill/execution-progress/` is the portable user Skill source. `agentdock_control.rs` owns the single automatic stale-control path and uses AgentDock's own local MCP `acp_session close`; `main.rs` owns lifecycle.

`src/main.tsx`, `types.ts`, and `styles.css` implement the desktop UI. `extension/` is an unpacked MV3 Chrome/Edge extension. `scripts/Register-AgentOrigin.ps1` is an optional source-side explicit-registration helper. `scripts/qa-native.js` is compiled into debug-only acceptance builds; it is not injected by the release.

## Attribution status — be precise

Exact **conversation → user-message Request → structured AgentDock invocation/result** attribution is implemented for newly observed desktop-browser turns. Each user message ID forms a Request independently of whether it launches ACP. Direct AgentDock tools can attach bounded execution evidence to that Request. `task_manage` result task IDs attach the declarative plan; multiple task IDs in the same Request become dynamic plan revisions. When ACP is launched, the stricter chain continues as exact ACP ID → `remote_session_id` → Codex thread, and exact subagent spawn edges can inherit that Agent evidence. Conflicts are rejected, not silently reassigned.

The current parser/bridge path is covered by 28 Node/browser tests plus installed runtime checks. 0.5.1 proved a real authenticated Chrome Request can reach `chat_requests`; 0.5.2 adds exact current-conversation mapping backfill for `task_manage` and ordinary AgentDock tools, including interruption/alternate transport cases. The 0.5.2 installed extension must be Reloaded once in Chrome and the ChatGPT page refreshed before claiming live Task/Steps/tool backfill. Pair again only if the extension itself reports pairing is missing.

The extension keeps ACP launch evidence strict: ordinary `read_file`, listings or text containing ACP-looking IDs cannot become ACP bindings. For ordinary AgentDock Request activity it records only bounded safe metadata such as tool name/action and project/workdir/path/task IDs; tool stdout and arbitrary command/file bodies are not copied into Request metadata. Private delta/patch or undocumented connector-envelope variants can still remain unsupported and should fail visibly rather than be reconstructed by time proximity.

Phone conversations cannot be observed by this Windows browser extension. Historical source IDs absent from captured data cannot be reconstructed from time/cwd similarity. An explicit local helper or future invocation-scoped AgentDock metadata is the honest fallback. No AgentDock telemetry modifications were made; no private browser profile/cookie extraction was used.

## Data, performance and limitations

Source roots default to `%USERPROFILE%\.codex` and `%USERPROFILE%\.agentdock`; optional environment overrides exist for development. Own data lives under `%LOCALAPPDATA%\AgentMonitor`. Key material, SQLite journals, probes and screenshots are excluded from Git. Review `docs/SECURITY.md` before exposing any new endpoint or diagnostic export.

The collector sleeps 2.5 seconds after each collection, so actual cadence is collection time plus sleep. Initial observed collection was about 3.6 seconds; an earlier warm probe was about 1.3 seconds. Do not claim a fixed 2.5-second end-to-end latency. Source metadata is capped at 15,000 rows, initial rollouts at 1 MiB, item detail at 4,000 rows. The UI calls out partial coverage.

Today/7-day/30-day tokens are positive deltas only during continuous observation. Lifetime baseline, resets, >30-second gaps and day-crossing gaps do not become today's usage. Conversation-by-time historical apportionment is not fully implemented and must not be invented. Tool wall-clock intervals are unioned; remaining gaps are unclassified, not assumed model time.

Further hardening deserves separate tests: prolonged soak/load, torn ACP JSON writes, owner-loss state across logical-turn restarts, unrecognized SQLite status/schema evolution, extension response variants, and full installer upgrade/uninstall behavior. Do not call this public-distribution hardened or code-signed.

## Reproduce / test

```powershell
npm ci
npm run build
npm test
cargo test --manifest-path src-tauri/Cargo.toml
npm run desktop:build
```

The current installer is `src-tauri\target\release\bundle\nsis\Agent Monitor_0.5.2_x64-setup.exe`. For source probes: executable `--probe .local\probe.json --twice`.

For native QA, build via `npm run tauri -- build --debug --no-bundle` (not a standalone debug executable expecting an absent Vite server), launch with `--qa-native`, and observe `%LOCALAPPDATA%\AgentMonitor\native-qa.json`. Keep the app alive for the lifetime of the verification command. AgentDock execution-session cleanup can end child GUIs; a Start-Process return by itself does not prove the installed app remains running. Screenshots need only the app's own client area, with temporary foreground placement restored afterwards.

WebView2 external CDP debugging was unavailable in the earlier elevated environment. Do not disable security or downgrade the runtime to recover it. The debug-only on-page-load harness remains earlier frontend/IPC evidence. In 0.5.0 the **installed** HTTP bridge/request endpoints were exercised directly with the real local pairing credential and extension-origin boundary: direct Request/task linking and a two-plan dynamic revision both appeared in the real installed snapshot. Those synthetic rows were deleted immediately after verification; this endpoint acceptance is not evidence that the authenticated browser extension has observed a real 0.5.0 ChatGPT turn.

## Next maintenance priority

First reload/update the unpacked extension in the owner's intended Chrome/Edge profile and refresh ChatGPT. Perform one real **ChatGPT user message → Request → direct AgentDock/task_manage → checkpoints** acceptance and confirm the Request/steps update live; then perform one real ACP launch when convenient to confirm the Agent branch remains attached to the same Request model. Pay special attention to real multi-stream ChatGPT responses around tool calls. After that, improve ETA/history from accumulated step durations rather than inventing estimates, and keep strict unknown/fallback behavior.

Update this handoff and relevant architecture/verification notes whenever behavior changes. Treat the local project as authoritative unless the owner explicitly asks for Git work; do not push to an invented remote.

## Historical installed-release note

The original 0.1.0 installer acceptance remains recorded in `docs/VERIFICATION.md`; it is historical and does not describe current runtime settings. The current installed/runtime facts are the 0.5.2 section at the top of this handoff and `docs/VERIFICATION_0_5_2.md`; `docs/VERIFICATION_0_5_1.md` records the preceding Chrome injection fix.
