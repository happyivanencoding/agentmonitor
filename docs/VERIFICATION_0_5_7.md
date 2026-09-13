# Agent Monitor 0.5.7 — Onward recruitment pipeline dashboard

Verified locally on 2026-09-13 against the installed per-user Agent Monitor and the real local Onward job-data pipeline. This release does not change AgentDock/Codex attribution semantics. The browser attribution extension remains 0.5.6 because its code is unchanged; no extension reload is required for this dashboard release.

## Scope

0.5.7 adds **Onward 招聘** as a peer navigation destination on desktop and the installable phone/PWA view. The dashboard is read-only and visualizes the existing Onward French job-market pipeline:

- four Windows scheduled stages: Raw collectors → Finance official collectors → Derived France Job Index → VPS incremental sync;
- provider/source health (`success / running / waiting / failed`) for France Travail, SmartRecruiters, Greenhouse, Ashby, Lever, Workable, Arbeitnow and finance-official sources;
- latest provider batch additions when the collector log exposes `saved N recent`;
- recent Raw ingestion deltas and VPS `upsert / delete` deltas;
- canonical/14-day totals, Paris/Île-de-France, Stage/Alternance, contract, industry and provider distributions;
- 14 finance-official source cards, including BPCE/Natixis, BNP Paribas, Société Générale, Amundi, CACEIS, CA-CIB, AXA, Citi, HSBC, BlackRock, Arkéa, Ardian, Rothschild & Co and Tikehau Capital;
- VPS search-index health based on the latest successful real import, scheduled-sync result/freshness, active job count and SQLite size.

The dashboard polls only while visible, every 15 seconds. It reads bounded derived/control data only; it does not scan the Raw Layer, the canonical JSONL, the 1+ GB VPS SQLite index or vacancy descriptions.

## Hidden Onward scheduled tasks

The four real Onward Windows tasks previously launched `.cmd`/PowerShell directly with `Hidden=False`, which could flash an empty console window on every 3/15-minute trigger. They now execute through `C:\Windows\System32\wscript.exe` → `C:\dev\onward-job-data\run-hidden.vbs`, whose `WScript.Shell.Run(..., 0, True)` starts the original command with a hidden window and preserves its exit code. Task Scheduler `Hidden=True` is also set. `MultipleInstances=IgnoreNew`, triggers, cooldown logic and original scripts were preserved. Original task XML was backed up under `C:\dev\onward-job-data\task-backups`.

Post-change real Task Scheduler readback showed all four actions using `wscript.exe`, all four `Hidden=true`, and all four latest results `0`. Representative real runs after the change included Raw at 14:14, Derived at 14:00, Finance at 14:12 and VPS Sync at 14:07 Europe/Paris. Structural headless evidence is therefore present; automated tests do not claim to visually observe the user's desktop for an extended period.

## Read-only/security boundary

Rust module `src-tauri/src/onward_jobs.rs` reads only:

- `C:\dev\onward-job-data\state.json`;
- `finance-official-state.json`;
- `derived/summary-14d.json` and `derived/build-state.json`;
- bounded tails of collector/finance/build/sync logs;
- the four fixed Task Scheduler entries through Windows Schedule.Service COM.

The Task Scheduler helper is a fixed PowerShell command launched with `CREATE_NO_WINDOW`. No arbitrary task name, command, file path or shell input is accepted from the UI. The native command and `/api/onward-jobs` route are read-only. Raw snapshots, Candidate/CV data, provider/API credentials and SSH keys are never returned.

## Validation

- Onward-specific Rust tests: 4/4 passed, including a real local snapshot assertion requiring all four scheduled tasks to be present/hidden and VPS health to be true.
- Full Rust suite: 50/50 passed.
- Node/browser attribution suite: 44/44 passed.
- TypeScript/Vite production build: passed.
- Optimized Tauri/NSIS build: passed; produced `src-tauri\target\release\bundle\nsis\Agent Monitor_0.5.7_x64-setup.exe`.
- Silent per-user upgrade: exit 0. Installed executable file/product version and `/health` report 0.5.7.
- Installed `/api/onward-jobs` real-data readback: available=true, four hidden/non-missing tasks, zero failed tasks, VPS `connected/healthy`, and non-zero canonical/source/finance metrics.
- Installed desktop browser acceptance: same-level **Onward 招聘** navigation opens the live dashboard with no console errors; the lower finance section also renders.
- Installed 390×844 mobile/PWA viewport: **招聘** bottom-navigation entry opens the same live dashboard with no console errors.

At verification time the real dashboard reported roughly 314k canonical indexed jobs, 237k unique jobs in the 14-day market view, 580 Stage, 7.2k Alternance, eight high-level source groups and 14 finance-official source cards. These are live observations, not hard-coded fixtures or stable market guarantees.
