# Verification — 2026-09-06

## Passed on the actual Windows machine

- TypeScript type checking and Vite production frontend build.
- Rust build/check and **23 Rust unit tests**: source read-only writes rejected, schema drift becomes unknown, status/terminal/warmup boundaries, credential redaction, baseline/gap/reset accounting, URL and binding conflicts, JSONL lifecycle/tool pairing, and bridge validation helpers.
- **16 Node attribution-parser tests**: exact ACP/remote-thread matching, actual parent-linked mapping, cross-conversation conflicts, no attribution from prose/listings/ordinary command stdout, no secrets in extracted metadata.
- A two-cycle native executable probe loaded a substantial local thread/ACP history and multiple monitored logical work items, with exact live lock-owner mappings at verification time. AgentDock health was reached through its discovered local runtime metadata.
- A sampled completed thread verified model/reasoning/token extraction and the rollout fallback path even though no turn-history projection row existed. Exact local counts, token totals and identifiers are intentionally omitted from the public verification note. The probe did not create fixture Agents in the UI.
- The actual embedded Tauri frontend, at `http://tauri.localhost/`, passed the debug-only native acceptance harness. It checked six-page navigation, dark/light rendering, Agent detail/evidence/timeline navigation, live snapshot advancement, real source read-only status, explicit binding visibility, invalid URL rejection, conflict rejection, and removal of temporary QA bindings. No frontend runtime errors or global horizontal overflow were observed.
- Optional autostart remained **off** throughout acceptance.
- An optimized x64 executable and NSIS per-user installer were produced. The release executable displayed the real dashboard without a Vite server.

The native acceptance exercised a substantial local work history and measured both cold and warmed collection paths. Exact local item counts and timings are intentionally omitted from the public verification note; they are observations rather than latency guarantees or performance benchmarks.

## Evidence locations (private, ignored)

- `.local/rust-tests-final.log`
- `.local/extension-tests.log`
- `.local/probe.json` and `.local/probe-data/`
- `.local/native-verification.json`
- `.local/native-capture-final.log` and `.local/ui/` screenshots
- `%LOCALAPPDATA%\AgentMonitor\native-qa.json`
- `.local/release-final.log`

Do not commit or upload the private snapshots/screenshots as public sample data. Unit-test fixture identities are synthetic. Native QA's temporary conversations are explicitly labeled, deleted, and checked for cleanup.

## Not claimed as verified

The extension has not yet been installed into the user's authenticated Chrome/Edge profile, and automatic extraction from the current ChatGPT streaming/patch encoding is not an accepted end-to-end capability yet. Phone and uninstrumented browser coverage is absent. Explicit desktop attribution is verified, not a substitute for that outstanding automatic-capture acceptance.

`scripts/verify-bridge.cjs` contains a real HTTP boundary test suite, but its launch was blocked by the tool execution safety check after an earlier attempt found the app was no longer running. Do not count it as passed. The native binding API round-trip and unit-level Host/origin/auth/body validation coverage are separate evidence. This is not a penetration-test certification.

CDP-driven Playwright acceptance was unavailable for the elevated WebView2 environment. No browser downgrade or security bypass was used. A debug-only in-process page-load harness successfully verified the same real frontend/Rust IPC instead. Screenshots temporarily put only the Monitor window on top to avoid capturing unrelated windows, then restore normal z-order.

Long-duration soak testing, every undocumented source schema variant, every browser response shape, signed distribution, upgrade/uninstall lifecycle and full historical analytics remain outside this first acceptance.

## Installation acceptance

The final NSIS installer exited with code **0** and registered Agent Monitor **0.1.0** for the current user. Installation directory: `%LOCALAPPDATA%\Agent Monitor`. The Start menu shortcut, bundled `extension/manifest.json`, application executable and uninstaller all exist. The installed executable was launched, rendered its real native window, and responded successfully on its local `/health` endpoint. The bounded test launch was closed afterwards; open the Start menu shortcut for normal use. Autostart remains off.

Installed payload comparison passed after accounting for the expected three-byte Tauri bundle marker (`NSS` in the installed binary versus `UNK` in the unbundled build). No other executable bytes differ. The installer is **3,451,192 bytes**, SHA-256 `324F4F678FF91460F00507219917A587E47E8BE1EF8A6A80D24EF7920BBD7059`. Private machine-specific evidence is in `.local/installation-verification.json` and `.local/ui/installed-overview.png`.
