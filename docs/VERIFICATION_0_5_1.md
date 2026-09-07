# Agent Monitor 0.5.1 — Chrome Request injection fix

## Failure reproduced

The first real 0.5.0 user message after extension reload still produced `chat_requests = 0` and `request_tools = 0`, while the extension background worker continued to heartbeat. Chrome history confirmed the ChatGPT page had actually been reloaded after the extension reload, so this was not a missed user refresh.

A controlled Chromium extension probe isolated the failure: a manifest content-script group beginning with a `.cjs` resource did not execute, while the same minimal extension using a `.js` resource injected normally on `https://chatgpt.com/`. The Agent Monitor manifest referenced `extractor.cjs`, so neither that MAIN-world extractor nor the following page observer ran.

## Fix

- browser manifest now loads `extractor.js`;
- Node tests execute the same browser source through `tests/load-extractor.cjs`;
- a regression test asserts all manifest content-script resources are `.js` and present;
- the page observer emits a liveness signal every 30 seconds;
- the bridge records `extensionVersion`, `pageObserver`, and `lastPageObserverSeen`;
- desktop settings distinguish background heartbeat from a recently live page observer.

## Verification before owner-browser reload

- Node/browser attribution tests: **26 / 26 passed**;
- Rust tests: **35 / 35 passed**;
- TypeScript/Vite production build: passed;
- Playwright Chromium using the actual extension directory against `chatgpt.com`: `AgentMonitorParser=true`, `observer=observer-ready`, manifest `0.5.1`;
- optimized NSIS package: `src-tauri\target\release\bundle\nsis\Agent Monitor_0.5.1_x64-setup.exe`;
- silent per-user install: exit 0;
- installed executable file/product version: `0.5.1`;
- installed loopback health: `{"app":"Agent Monitor","version":"0.5.1"}`.

## Remaining live acceptance

An unpacked Chrome extension keeps its currently loaded scripts until the user reloads the extension (or restarts Chrome). After installing 0.5.1, reload **Agent Monitor · ChatGPT attribution** once in `chrome://extensions/`, refresh the ChatGPT tab, then send one real user message. Live acceptance passes only when that real message appears in Monitor `chat_requests`, and structured AgentDock activity from the same turn appears under that Request when applicable.
