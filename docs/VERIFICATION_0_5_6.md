# Agent Monitor 0.5.6 verification

## Scope

0.5.6 fixes canonical project identity for temporary AgentDock model-transport workspaces. A directory under `.agentdock/tmp` named like `<project>-isolated-model-<pid>-<suffix>` is an execution workspace, not a top-level project. The collector now resolves it to a stable project key while preserving the exact temporary `cwd` as Agent evidence.

When the transport slug matches a real project folder under the same drive's `dev` root, that real folder name is used. Otherwise all matching temporary workspaces collapse to the transport slug. Historical usage rows are canonicalized while grouping, so old isolated-workspace labels do not recreate duplicate projects in analytics.

## Checks chosen

The targeted Rust tests were chosen because failures would change this implementation: project parser tests catch invalid or over-broad workspace rewriting, and the analytics test catches failure to merge historical aliases before ranking.

- project canonicalization tests: **4 passed**;
- historical project-usage merge test: **1 passed**;
- TypeScript/Vite production build: passed;
- optimized Tauri/NSIS build: passed and produced the **0.5.6** per-user installer.

## Installed runtime verification

The 0.5.6 installer was applied to the existing per-user installation and only Agent Monitor itself was restarted. AgentDock and unrelated model sessions were not deliberately restarted or stopped.

The installed loopback Web bundle contains the 0.5.6 frontend version. A real installed snapshot reported no top-level project keys containing `isolated-model`, while observed matching temporary workspaces retained their original `cwd` and resolved to their canonical project. The 30-day usage response likewise reported no `isolated-model` project labels after aggregation, confirming that historical stored aliases are merged at read time.

The configured public hostname remained reachable and returned the expected Cloudflare Access redirect to an unauthenticated request. This verifies the public route is online; it is not a substitute for an authenticated browser session.

## Boundaries

This change canonicalizes recognized AgentDock model-transport workspace names only. Other temporary directory naming conventions remain untouched until they have explicit evidence linking them to a project. Exact conversation attribution rules are unchanged, and a project match does not imply that two Agents belong to the same ChatGPT conversation or Request.
