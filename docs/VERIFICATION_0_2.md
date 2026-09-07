# Agent Monitor 0.2.0 — release acceptance

## 0.2.1 focused delta — archive / restore

The 0.2.1 change is intentionally narrow: an owner acknowledgement can archive an attention item without falsifying its observed Agent status. `npm run build` passed and `cargo test --manifest-path src-tauri/Cargo.toml` passed 29/29 tests, including the new archive persistence/restore round-trip. `npm run desktop:build` produced `Agent Monitor_0.2.1_x64-setup.exe`, which was installed for the current user.

After installation, the running local collector reported version `0.2.1`. The eight attention records the owner explicitly identified were archived through the implemented local API. A later independent observation, after the temporary launch task had already been deleted, still found the installed process alive and reported `stalled=0`, `failed=0`, `archived=8`. The two failed rows retained `status=FAILED`; the other selected historical rows retained their then-current observed/inferred status. This verifies that archive changes Monitor disposition rather than source lifecycle state. No continue/retry/stop/restart Agent control was added.

Verified on 2026-09-06 against the user's actual Windows data and installed application.

## Installation and continued operation

The per-user NSIS upgrade exited with code 0. The installed executable reports 0.2.0 and the installation contains the web assets, PWA manifest, service worker, icons and original browser extension.

A temporary on-demand Windows task launched the installed tray application outside the command runner's process cleanup. The task was then removed. A later, separate command confirmed that the installed process remained alive and responsive after launcher and browser-test commands had ended. The web endpoint reported version 0.2.0 and a fresh, non-stale snapshot containing substantial local work history; the exact local item count is omitted from the public note.

The normal current-user `AgentMonitor` Windows Run entry now points to the installed executable with `--minimized`. No permanent scheduler task or duplicate login trigger was left behind. The app is intentionally left running, unlike the historical 0.1.0 launch-only test.

## Automated and interactive evidence

**Rust: 28 tests passed.** This includes the existing collector/state/storage tests and the added Access validation configuration, forged-algorithm rejection, forwarded-loopback, static-path and public-host boundaries.

**Attribution parser: 16 tests passed.** These retain the original prohibition on guessing launch provenance from listings, prose or timestamps.

**Updated native Tauri acceptance: passed.** The real embedded frontend loaded a substantial local logical-work/ACP/Codex history. Model/token/rollout fallback, multiple lock-owner mappings, refresh, explicit binding round-trip, conflict rejection and cleanup passed, without JavaScript errors or global overflow. Exact local record counts are omitted from the public verification note.

**Installed web/mobile acceptance: passed.** A real browser loaded the installed assets and running collector, navigated all six pages at desktop size and 360/390/412 px widths, exercised theme and installation controls, and opened actual Agent details/evidence/timelines. The mobile binding dialog saved a clearly labeled temporary binding into the shared collector. Conflicting reassignment was rejected and the temporary evidence was removed.

The first mobile test exposed a real stacking defect: the binding dialog was behind full-screen details. It was corrected, and the actual click/save test subsequently passed. Stable-frame screenshots were visually inspected; the compact overview exposes an Agent above the bottom navigation, and the details page is readable and full-screen.

**Refresh/PWA behavior: passed.** The real collector progressed. The manifest defines standalone installation and both required icon sizes are present. Only UI/offline resources entered CacheStorage. Network loss changed LIVE to OFFLINE, reload displayed no cached Agent data, and reconnection restored real snapshots. An injected 401 verified expired-login presentation/recovery; this injection is not a genuine Cloudflare login test.

**HTTP/public boundary: 16 checks passed.** The configured public Host requires a signed Access token; the identity email header alone cannot authenticate. Forwarding headers cannot turn tunnel traffic into local access. Foreign origins, missing mutation headers and oversized bodies are rejected. Pairing secrets, remote configuration and native command endpoints are absent from the route table. Actual anonymous public HTTPS requests to both `/` and `/api/snapshot` returned 302 to the configured Cloudflare Access login host.

The installed snapshot measured about 975 KB uncompressed and 153 KB with gzip. An unchanged observation version returned a body-free 204. No content hash or persistent browser data cache was introduced.

## Evidence locations

Private reports and screenshots are excluded from Git:

```text
.local/remote-rust-tests.log
.local/remote-parser-tests.log
.local/remote-native-qa.json
.local/remote-installation.json
.local/web-qa/report.json
.local/web-qa/boundary-report.json
.local/web-qa/*.png
```

## Acceptance limits

The control-plane credential could create the Access app/policy and DNS/Tunnel route but could not create an Access service token (403). No bypass was added. The owner has not yet completed an observed public login on a physical phone, and a real phone home-screen installation has not been claimed. The phone release is a PWA, not an APK.

Long-duration soak, physical-device keyboard/OS sleep behavior, background push notifications and native mobile packages are not covered. Original ChatGPT launch-attribution limitations remain unchanged: remote viewing does not invent provenance for unassigned calls. The Windows app must be running on an awake, connected computer.
