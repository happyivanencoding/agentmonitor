# Agent Monitor 0.4.0 — automatic tasks and stale cleanup verification

Verified on 2026-09-06 against the current Windows source tree and installed application.

## Automatic task attribution

The bundled ChatGPT extension now attaches the causal user turn to a recognized ACP launch. For a full conversation mapping it walks the real parent chain from the tool result through the structured assistant invocation to the nearest user message; for a live request it reads the current request's user turn. It does not infer across conversations by time, cwd, project name, or shared AgentDock parent namespace.

Monitor stores an automatic `monitor_task` only when that user turn actually has recognized ACP launch evidence. Multiple ACP sessions may attach to one Monitor Task. Continuation/modification wording reuses the current task; explicit new-goal wording creates a new one. Pure queries that launch no ACP do not create tasks. The task board keeps these Monitor Tasks distinct from AgentDock native Task objects.

`npm test` passed 20/20 attribution parser tests. New coverage includes request-user-turn extraction and exact parent-chain user-turn association; existing concurrent-tab and one-conversation/multiple-ACP tests remain green. `cargo test --manifest-path src-tauri/Cargo.toml` passed 33/33 tests, including automatic task reuse/split behavior and the exact 24-hour stale threshold.

## Task board

The navigation item is now **任务看板**. It merges Monitor Tasks and AgentDock native Tasks, groups rows by project, sorts by creation time, and renders a horizontal 7/30/90-day timeline. TypeScript/Vite production compilation passed. The existing Agent detail still shows an explicitly linked AgentDock native Task when one exists; automatic Monitor Tasks are a separate user-goal layer.

The work overview / **全部记录** and global timeline now use the same collapsible hierarchy: project folder first, then ChatGPT conversation + Task, then individual Agent rows. Conversation/task groups are collapsed by default; project groups can also be collapsed. The timeline shows one summary span for the grouped work until the user expands its individual Agents. This directly addresses the previous flat list where agents from several concurrent projects were difficult to distinguish.

## 24-hour stale cleanup

The desktop collector starts a local cleanup loop. When a logical Agent has had no activity for more than 24 hours and still owns an open AgentDock ACP session, it calls AgentDock's real local MCP tool `acp_session` with `action=close`. It records success/failure in Monitor's own SQLite store and archives the owning logical thread after a successful close. It does not terminate the shared Codex host process. Codex-only stale records have no safe per-thread kill primitive and are therefore expired from live/current/timeline views rather than killing unrelated logical threads.

The global timeline and current/attention scopes now independently exclude archived or more-than-24-hour inactive rows, so historical stale turns cannot continue rendering as multi-day active bars while cleanup completes.

Live acceptance used the installed 0.4.0 application and the local AgentDock session store. The cleanup sweep closed stale sessions older than the threshold with zero observed failures, and a direct follow-up scan found no remaining open sessions beyond 24 hours. Exact local session IDs, timestamps and counts are omitted from the public verification note.

## Build and installation

`npm run build` passed. `npm test` passed 20/20. `cargo test --manifest-path src-tauri/Cargo.toml` passed 33/33. `npm run desktop:build` produced:

```text
src-tauri\target\release\bundle\nsis\Agent Monitor_0.4.0_x64-setup.exe
```

The silent per-user upgrade exited 0. The installed executable was launched using a temporary one-shot scheduled task, which was deleted immediately after launch. Both `http://127.0.0.1:43217/health` and the real snapshot API reported version `0.4.0`; the installed process remained running while the real stale cleanup executed.

After the final grouped-directory/timeline changes, the 0.4.0 installer was rebuilt and installed again. The installed web bundle contained both the project hierarchy label (`项目 → 对话 / 任务 → Agent`) and the conversation/task grouping label (`个对话 / 任务`), confirming that the running installed release contains the collapsible grouping rather than an earlier 0.4.0 build.

## Remaining browser acceptance

Automatic Task creation depends on the same one-time Chrome/Edge unpacked-extension load and local pairing required by ChatGPT attribution. Parser/unit/build acceptance is complete, but do not claim that the user's authenticated ChatGPT browser has produced a live 0.4.0 Monitor Task until the extension is loaded/updated, paired, the ChatGPT tab refreshed, and a real new ACP launch creates a `monitorTasks` entry without manual binding.
