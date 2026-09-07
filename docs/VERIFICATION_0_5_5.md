# 0.5.5 — independent task-progress latency acceptance

Date: 2026-09-07. This supplements [0.5.4 capture acceptance](VERIFICATION_0_5_4.md); all of those capture, privacy, exact-association and client-coverage boundaries remain applicable.

## Additional defect found during closeout

The real heavy collector required about **41.7 seconds** in one observation and **57.461 seconds** during the latency test. The previous loop waited for process/lock, Codex and token collection before publishing Task changes. Its nominal 2.5-second sleep was not a 2.5-second end-to-end refresh interval. A real completed Task therefore temporarily remained active in the visible snapshot despite correct persisted source state.

## Repair

`src-tauri/src/progress.rs` reads native Task projections and Monitor-owned Request/tool rows independently every two seconds. It does not scan process locks, Codex rollouts or token histories. This path updates only exact existing Task references and does not invent browser ownership. New observed Request rows can reach the UI without waiting for a full Agent scan; Agent/thread joins remain the heavy collector's responsibility.

Before publishing, the heavy collector preserves newer progress and Request records. Desktop and web consumers use progress/full-publication timestamps as revisions and reject older deliveries. Full metrics keep their actual `generatedAt` sample time rather than being retimestamped as fresh. `collectionCompletedAt` is publication time, not a claim that every metric was sampled at that instant.

The UI distinguishes current steps from older Agent/process/token metrics. On cold start, actual local Tasks can appear before the first heavy collection finishes; `metricsLoading` explicitly describes that partial state. The frontend footer now matches release 0.5.5 instead of the old hard-coded 0.5.2 string.

## Real latency experiment

`scripts/verify-progress-latency.cjs <actual-task-id> <actual-step-id>` was started before a normal AgentDock checkpoint completed an implementation step and entered the latency-test step. This read-only probe wrote no Task, synthetic Request or fabricated status.

| Observation | Actual result |
|---|---|
| Installed Monitor version | 0.5.5 |
| Real checkpoint first visible through Monitor API | **777 ms** after the persisted Task update |
| Concurrent full collection duration | **57,461 ms** |
| Following complete heavy-collector publication | New completed step remained current; no regression |
| Native plan UI at desktop and 420-pixel mobile viewport | Real 1/3 steps and the current step matched source state |
| UI errors / global mobile horizontal overflow | None observed |

777 ms is one observed checkpoint-to-API latency, not a universal SLA or a measurement of phone network/rendering latency. The server's light refresh is two seconds; the web client polls approximately every three seconds, and desktop receives snapshot events. File-system, scheduling and network delays can add latency. The heavy collector was **not** made instantaneous; the step path was separated from it.

The actual timing artifact and screenshots remain in ignored `.local` files. Public source uses only fixture IDs or command arguments, never a real conversation, Task or browser-installation ID.

## Tests and deployment

- **44 Node tests passed**, retaining exact message/connector parsing, ownership restrictions, retry and browser-isolation coverage.
- **41 Rust tests passed**, including five new fast-path tests: exact Request updates without changing metrics age, completed/blocked steps, slow-snapshot non-regression, partial cold start and new Request/tool publication.
- TypeScript/Vite production build, Tauri release executable and the **0.5.5 NSIS installer** passed.
- The real Chromium extension test with explicitly synthetic transport passed on version 0.5.5. It remains distinct from signed-in browser evidence.
- The real installed native-plan UI verification passed at desktop/mobile viewports.

The installed per-user Monitor and bundled extension were updated without restarting the shared AgentDock service, closing user ChatGPT conversations, modifying other business projects or deliberately stopping their ACP/Codex agents. Existing paired credentials and Monitor data were retained.

## Boundaries unchanged

At the final delivery read, the installed app and active extension were both 0.5.5, the database held **four Requests and sixteen tool records**, and the latest observer report was `mapping-tools-observed` with an empty capture error. This is a latest-observation diagnostic, not an audit of every browser tab. The previously recovered real browser Request still has seven observed tool records and an exact checkpoint relation to a 3/3-step native Task. The legacy create result was not recovered, so the relation is labeled checkpoint participation rather than creation. Phone/native chat text and unopened conversations are not account-wide captured by this desktop extension; their real local Tasks can still display progress independently. Undeclared steps cannot be inferred from hidden model reasoning.

A ChatGPT mapping HTTP 429/503 still causes an explicit rate-limit diagnostic and `Retry-After` backoff. This does not stop the local Task refresh path. Native progress freshness must not be mistaken for proof that every browser tab's mapping is currently available.

### Later closeout observation — browser acceptance remains partial

A later sample contained **six Requests and sixteen tool records**, but its most recent observer diagnostic was `mapping-unavailable` with `authenticated missing-exact-mapping, f:404`. New Request rows continued to arrive; a complete matching tool/plan mapping was not confirmed for every one of them. Do not treat the earlier successful observer report as universal or permanent browser acceptance. This observation alone does not distinguish an in-flight mapping not yet available from an unsupported exact-ID/mapping variant, and that distinction was not resolved in this release.

Independent native progress remained correct: both repair Tasks reached completed status (6/6 and 3/3). A separate read-only probe observed the final Task completion through Monitor after **1,823 ms**. Thus native step/completion freshness is verified, while full tool-level capture across all new conversations remains unverified. Continue diagnosis from exact message/conversation evidence rather than guessing ownership or restarting unrelated agents.
