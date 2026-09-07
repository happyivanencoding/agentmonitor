# 0.5.4 — request capture and native progress acceptance

Date: 2026-09-07. This report contains aggregate evidence only. Real conversation IDs, Task IDs, connector installation IDs, prompts, account data, screenshots and database backups remain in ignored local artifacts.

## Failure recovered

The starting installation and the browser extension were both 0.5.2. The paired observer reported readiness, but Monitor contained only **two Requests and zero tool records**. It was not accurate to attribute this solely to an extension that had not been reloaded. The supplied shared conversation was read in full to the extent the sharing platform exposed it; plugin invocation/results in that export were redacted. Local runtime files and real browser capture, not invented redacted outputs, were used for diagnosis.

Concrete defects repaired:

- DOM fallback depended on request text and later mapping success. It now captures exact message IDs, delayed new-conversation URLs, repeated text, and attachment-only requests.
- Mapping ancestry stopped after 60 nodes. Long development turns now retain exact causal attribution with cycle detection.
- Actual connector paths can include `AgentDock/link_<opaque>/task_manage`. They are canonicalized before tool semantics are applied. Known code/text result envelopes and actual `recipient=all` plus explicit end-turn completion are recognized.
- The old mapping path silently failed. Same-origin authenticated reads, passive JSON mapping processing and explicit diagnostics now cover supported fallback paths. A missing or unreadable result stays unreadable; no Task or ACP ID is invented.
- Events were effectively treated as sent before durable delivery. Sanitized observations now use an acknowledged persistent worker outbox and retry after Monitor outages. Replayed starts cannot reopen completed tools.
- Old injected observers can survive extension updates. Versioned observations are checked by the relay and background worker; the updater replaces the extension's own observers without refreshing ChatGPT pages.
- Native Tasks with no ACP were not usefully expandable. A separate local-plan worklist exposes real steps without requiring browser association or an ACP Agent.

## Real signed-in browser acceptance

A real pre-existing Request recovered **seven ordinary AgentDock tool records**. Explicit checkpoint invocations in that exact Request carried the real Task ID; the collector joined it to the existing native Task and displayed **3/3 completed steps**. This did not require launching ACP, entering a guessed ID, changing the Task, or injecting a test observation into the live database.

The legacy create result did not yield a usable Task ID. Therefore the recovered relation is explicitly **`checkpointed` / 显式 checkpoint 关联**, not a claim that its creation result was recovered. Reading or listing another Task does not create this association. A different open conversation's attachment-only message also became a Request, using its exact message identity.

This is evidence that the repaired live mapping and Task association work for these actual messages. It is **not** evidence that every old or unopened conversation was imported, nor that every private ChatGPT transport has been validated.

## Native live progress and UI

The real repair Task was visible independently of browser association with six steps. Its actual checkpoints moved the displayed state from **3/6, verification in progress** to **4/6, deployment in progress**, without creating an ACP Agent. The installed 0.5.4 web UI matched the collector's step count and completed-state classes at desktop and 420-pixel mobile widths. The native plan could be expanded without an Agent link; no page errors or global mobile horizontal overflow were observed.

The overview keeps the global timeline, then provides **本机执行计划** alongside attributed Requests. Unassigned native plans explicitly state that their ChatGPT conversation is unknown. Monitoring a local Task started by a phone/native client is possible; capturing all phone/native chat text is a different capability and is not claimed.

## Tests and build

| Verification | Result |
|---|---|
| `npm test` | 44 Node tests passed |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 36 Rust tests passed |
| TypeScript / Vite production build | Passed |
| Tauri release executable and NSIS installer | Built as 0.5.4 |
| `scripts/verify-capture.cjs` | Real Chromium extension with synthetic transport: authentication, metadata-only capture, Task result, completion, identical text with distinct IDs, reinjection and two-tab isolation passed |
| `scripts/verify-native-plans.cjs <actual-local-task-id>` | Actual installed 0.5.4 collector and desktop/mobile UI passed |
| Signed-in owner-browser recovery | Seven tools and exact checkpoint relation to 3/3 steps observed |

Synthetic browser test payloads stay in an isolated browser profile and do not enter the production Monitor database. The real native verification takes a local Task ID as an argument; it does not hard-code private IDs into source. A first native test attempt ran during collector initialization and correctly failed rather than claiming an initialized snapshot; it passed after the actual snapshot became ready.

## Deployment and continuity

The per-user 0.5.4 desktop executable and bundled extension were installed and their actual versions checked. The existing Monitor database was backed up before the upgrade and retained. Only the Monitor process was stopped for installation. The running AgentDock service and the original Chrome browser process were preserved; no TGN or other business-project code was changed and no ACP/Codex session was stopped for this repair. The existing execution-progress user Skill remains the semantic-plan policy.

Subsequent extension-only changes in this task add `Retry-After` handling: mapping HTTP 429/503 pauses active fallback reads for the service-specified delay (at least one minute), rather than trying an alternate endpoint immediately. The final installer includes these resources. A temporarily rate-limited web mapping is displayed as such; local Task collection remains independent. Liveness is not used to conceal a rate-limit or unsupported mapping.

## Remaining limits

Browser evidence requires an instrumented desktop ChatGPT tab. An unopened conversation, a native/mobile ChatGPT message never exposed to that tab, or a workflow that emits no persistent Task cannot be reconstructed by guessing. Full account-wide chat synchronization is not implemented. Actual future live SSE variants and every supplemental-plan workflow have not all been exercised on the owner's account; supported structured messages and exact mapping fallback are tested separately. Plans expose only declared Task steps and checkpoints, not hidden model reasoning or invented percentage-complete.

The page keeps any same-origin session token in memory only. It is not sent to the extension worker, Monitor API or repository. Persisted capture metadata excludes ordinary stdout, command bodies, replacement file contents and raw transcripts. Unknown ownership remains unknown.
