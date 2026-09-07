# Security and privacy boundaries

## Local source data and optional protected remote viewing

The desktop process reads the current user's AgentDock and Codex directories. Codex SQLite connections use read-only flags, `query_only=ON`, a short busy timeout, and normal WAL-aware reads. Monitor writes only its own observation store under `%LOCALAPPDATA%\AgentMonitor` and explicit application preferences. There is no cloud database, account service, telemetry upload, user-exposed process-control endpoint or arbitrary command endpoint. The one background control action is the owner-requested 24-hour stale cleanup: it calls AgentDock's own local `acp_session close` for stale ACP sessions and records whether that close succeeded.

The process inventory never executes commands from monitored data. Windows Restart Manager is used only to query the owners of thread-writer lock files; it never invokes shutdown or restart APIs. The stale cleanup never kills a shared Codex host PID because one host can own multiple logical threads. Shared host children are labeled host-scoped rather than falsely assigned to a single thread.

## Sensitive source data

The collector does not retain full ChatGPT transcripts, private reasoning text, tool output, patches, browser cookies, or Codex authentication files. For automatic task identification the paired browser extension sends only the user turn that causally triggered a recognized ACP invocation, together with its message/conversation/ACP identifiers. Monitor stores a bounded task title and up to 500 characters of that triggering goal in its own SQLite task table; unrelated conversation turns are not copied. SQL projects only the remaining item metadata needed for observation. Normalized commands are length-limited and common credential patterns are redacted before journal storage. Pattern-based redaction is not a guarantee that every possible secret embedded in an arbitrary command will be recognized: keep the journal and screenshots private.

The SQLite journal is not encrypted by this application. It relies on the Windows user profile's filesystem access controls and the machine's disk protection. Anyone already able to execute code as this Windows user can inspect local data. Do not publish the runtime folder or change its ACLs to grant other users access.

## Attribution bridge

Only `127.0.0.1:43217` is bound. Mutation requires a random 64-hex-character bearer credential stored in `bridge.key`. The UI reveals/copies it only on an explicit click. Requests have an exact loopback Host allowlist, JSON content-type requirement, 16 KiB size cap, and Chrome-extension-origin CORS policy. Ordinary web origins, including the ChatGPT page itself, cannot directly use the key. The extension background worker holds the credential; page scripts do not receive it. The only unauthenticated read endpoint, `/health`, returns application name and version, not observed work.

The key is not printed by the registration helper or tests. Never paste it into a ChatGPT conversation, Git repository or support report. To rotate it, quit Monitor, remove its own `bridge.key`, restart, then pair the extension again.

Browser tool-result attribution is evidence from the paired browser, not cryptographic attestation by ChatGPT. Compromised page code could falsify metadata, but the bridge cannot execute programs, read arbitrary local files, stop agents, or modify source projects. Existing bindings to a different conversation are rejected until explicitly removed through the desktop UI.

## Desktop preferences and links

The frontend's native command surface allows only known Monitor operations. Chat links must be HTTPS on `chatgpt.com` and identify a valid conversation UUID. Folder-opening commands open fixed application folders. Autostart writes only the current user's Windows Run entry and is off by default. Closing the window leaves the collector in the tray; Quit ends it.

The first installer is unsigned. It is intended for this user's local installation, not a claim of publicly trusted distribution. A future public release should add code signing, installer upgrade/uninstall regression tests, dependency auditing and a separate security review.

## Development verification

`--qa-native` and optional CDP configuration are compiled only in debug builds. The native QA harness exercises the actual WebView and Rust IPC, not mock data. Temporary QA conversation bindings are visibly labeled and removed in a `finally` cleanup; successful tests also assert that none remain. Screenshots and real-data probes stay in ignored `.local/`.

The release does not enable remote debugging or load the QA script. Do not lower WebView2 security or roll it back to work around elevated-process debugging restrictions.

## 0.2 remote web boundary

Remote viewing is explicitly enabled for this deployment at `monitor.example.com`. Database files remain on the computer, but authenticated monitor metadata leaves it through Cloudflare. The new listener is separate from the extension bridge: `127.0.0.1:43218`, not 43217. A shared Cloudflare tunnel may route only the configured Agent Monitor hostname there. Existing unrelated application routes and login policies must remain unchanged.

A public request must pass Cloudflare Access and origin-side JWT validation. The origin validates signature, RS256, the configured team's issuer, the Monitor application's own audience, exp and nbf. It never trusts the email header alone or assumes that a loopback TCP peer is local when forwarding headers are present. The canonical loopback Host/no-forwarding path is available to the cooperating local machine user.

The remote route table is fixed: snapshot/detail/analytics/setup reads, plus explicit conversation/task metadata and Monitor-owned archive/restore writes. Archive changes only Agent Monitor's own acknowledgement state; it does not mutate AgentDock/Codex source state or control a process. There is no native IPC proxy, arbitrary file access, shell or process-control operation. Mutations require exact same Origin, JSON, an explicit client header and bounded bodies. API responses are no-store. PWA caches contain only static UI assets; precaching rejects redirects so expired login pages cannot replace assets. No extension pairing credential is exposed remotely.

The existing Cloudflare control-plane credential could not create an Access service token (403). No bypass rule was substituted. Automated verification covered local real-data browser interaction and public anonymous rejection; actual owner authentication on a phone is a separate acceptance step. See `REMOTE_ACCESS.md` for the operating model and limitations.

Primary reference: Cloudflare, Validate JWTs — https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/authorization-cookie/validating-json/
