# Agent Monitor 0.5.2 — per-Request plan and tool backfill verification

## Problem

A real Chrome Request was finally captured in 0.5.1, but the captured row still showed `无执行计划` even though the assistant had actually used AgentDock tools. Two separate causes were found:

1. the execution policy had resumed a Task created for an earlier user message instead of creating a new Task for the current Request;
2. some ChatGPT interruption/alternate response paths exposed the user message but did not expose structured AgentDock tool frames through the live response stream, so `request_tools` remained empty.

## Fix

- `execution-progress` upgraded to **1.0.1**: every user message that actually causes execution must create its own Task/Steps; short phrases such as “测试一下” are execution Requests when they trigger real tools;
- a new user message may not reuse the previous Request's Task as its current plan;
- the browser parser can inspect the exact current conversation mapping for one explicit user-message ID and backfill `task_manage` plus ordinary AgentDock tool activities;
- the page observer polls that mapping after a recognized request and also uses a DOM-triggered fallback for interruption/alternate transport paths;
- tool mapping stays causal: another user turn's tools are rejected by regression tests;
- while a Request is still responding and no plan has arrived yet the UI shows `等待执行计划`; observed tools without a claimed Task show `未认领计划`; only completed no-execution Requests show `无执行计划`.

## Verification

- browser/parser tests: **28 / 28 passed**;
- `node --check extension/page-observer.js` and `extractor.js`: passed;
- TypeScript/Vite production build: passed;
- optimized NSIS package: `src-tauri\target\release\bundle\nsis\Agent Monitor_0.5.2_x64-setup.exe`;
- silent per-user install: exit 0;
- installed executable file/product version: `0.5.2`;
- installed bridge health: `{"app":"Agent Monitor","version":"0.5.2"}`;
- user Skill `execution-progress` 1.0.1 validated, installed and activated; the mirror under `%USERPROFILE%\.agentdock\policies\global-execution.md` was updated.

## Live acceptance boundary

0.5.1 already produced a real authenticated Chrome `chat_requests` row for a short user test message, with `pageObserver=observer-ready` and extension version 0.5.1. That proved Request capture itself. The actual prompt text is intentionally omitted from the public verification note.

The 0.5.2 tool/Task backfill code is installed on disk but an unpacked Chrome extension must be Reloaded once after the upgrade. After Reload + ChatGPT refresh, the next execution Request should be accepted only when its row shows its own Task/Steps and the relevant AgentDock execution entries. Do not claim this final 0.5.2 browser acceptance before that observed row exists.
