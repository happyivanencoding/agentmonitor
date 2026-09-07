# Agent Monitor 0.3.0 — automatic ChatGPT attribution verification

Verified on 2026-09-06 against the current Windows source tree and installed application.

## What changed

0.3.0 turns the existing strict browser-attribution components into a first-class setup flow. The Windows app now exposes **连接与设置 → ChatGPT 自动归属**, can open the installed Chrome or Edge extension manager together with the exact bundled extension directory, shows whether the local bridge has seen the paired extension, and labels manual conversation binding as a fallback.

After one-time extension loading and pairing, the popup does not need to stay open. The extension heartbeats when the browser starts and when the ChatGPT observer becomes ready. A recognized AgentDock launch result is bound only as the exact `conversation_id` observed in that ChatGPT response plus the exact returned ACP ID; ACP `remote_session_id` then identifies the exact Codex thread. Unknown response shapes remain unassigned rather than being matched by time, cwd, project or shared parent namespace.

## Automated verification

`npm run build` passed. `npm test` passed 18/18 attribution parser tests. The suite includes explicit cases for two concurrent ChatGPT conversations producing different ACP sessions and for one ChatGPT conversation producing multiple distinct ACP sessions. Existing mismatch, parent-edge, list/status rejection and conflict-oriented tests remain green.

`cargo test --manifest-path src-tauri/Cargo.toml` passed 29/29 Rust tests. All four extension JavaScript files (`background.js`, `page-observer.js`, `relay.js`, `extractor.cjs`) passed `node --check`.

The machine has actual browser executables at `C:\Program Files\Google\Chrome\Application\chrome.exe` and `C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`, which are locations handled by the new native setup command.

`npm run desktop:build` produced `src-tauri\target\release\bundle\nsis\Agent Monitor_0.3.0_x64-setup.exe`. The silent per-user upgrade exited 0. The installed application was then launched outside the command runner's child-process lifetime using a temporary one-shot task, which was deleted after launch.

The installed attribution bridge health endpoint returned version `0.3.0`; the real snapshot API also returned version `0.3.0`, and `sources.bridge.healthy` was true. The installed resource `%LOCALAPPDATA%\Agent Monitor\extension\manifest.json` was present and reported extension version `0.3.0`, confirming the setup UI points at the packaged extension rather than the source tree.

## Remaining live acceptance

At the post-install check, `sources.bridge.lastExtensionSeen` was null. Therefore the bundled extension had not yet been loaded and paired in the user's intended authenticated Chrome/Edge profile, and no claim is made that current private ChatGPT response formats have already passed a live automatic-capture run.

The remaining owner action is one-time only: open **连接与设置 → ChatGPT 自动归属**, use the Chrome or Edge setup button, enable developer mode, load the already-opened unpacked extension directory, paste the local pairing key into the extension once, and refresh ChatGPT tabs that were already open. After that, a real ChatGPT → AgentDock launch should make `lastExtensionSeen` non-null and produce a `browser-tool-result` binding without manual ACP entry.

For strongest live concurrency acceptance, use three ChatGPT conversations at the same time and launch five ACP sessions total, including two or more from one conversation. Expected result: three distinct conversation groups, five exact ACP identities, five exact Codex threads, and zero cross-conversation reassignment. Any unrecognized invocation may remain unassigned; it must never be filled by timing heuristics.
