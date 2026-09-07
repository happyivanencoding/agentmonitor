# Agent Monitor 0.2 — web and mobile access

## Addresses and operating model

- Public: `https://monitor.example.com`
- Same-machine browser: `http://127.0.0.1:43218`
- Windows application: `%LOCALAPPDATA%\Agent Monitor\agent-monitor.exe`
- Runtime data and remote configuration: `%LOCALAPPDATA%\AgentMonitor`

One Windows process owns one collector, the existing Tauri window/tray, the original extension attribution bridge on port 43217, and the new web server on port 43218. There is no second agent database, extra cloud collector, Node server, or duplicate token counter. Both the native IPC and web HTTP operations call the same Rust API functions.

Source databases stay on the home computer. Remote viewing transmits monitor metadata, including titles, paths and bounded/redacted commands, through the authenticated Cloudflare connection. This is not a claim that no data ever leaves the computer. The web API is not a general filesystem or shell proxy.

The Windows user must be logged in and Monitor must be running. Closing the window leaves it in the tray; **Quit** stops both local and remote monitoring. The computer must remain powered on, connected and awake. Tunnel connectivity does not wake a sleeping computer, and this version does not override Windows power settings. Automatic Windows login startup is the existing current-user `AgentMonitor` Run entry, managed by the desktop preference; it is not a pre-login Windows service.

## Owner login and phone installation

The hostname should have its own Cloudflare Access self-hosted application. Its Allow policy uses the owner email supplied through the local `AGENT_MONITOR_OWNER_EMAIL` environment variable. No unrelated identity policy should be reused, and no anonymous or bypass policy is required.

Open the public URL, complete the existing Cloudflare login flow using the authorized owner email, and then open **连接与设置 → 安装 / 添加到主屏幕**. On Android Chrome, the browser may offer **Install app** or **Add to Home screen**. On iPhone Safari, use **Share → Add to Home Screen**. Browser UI wording and installation prompts vary.

This release's phone application is an **installable PWA**, not an Android APK or an iOS App Store build. It has a standalone manifest, 192/512 icons, safe-area-aware mobile navigation, full-screen agent details, touch-sized controls, theme support and an offline page. It shares the web release, so no separate mobile business logic needs to be synchronized.

Only UI assets and the offline explanation are cached. Agent snapshots, commands, tasks, identities, Access responses and API data are not persisted in browser caches. An already-open page may retain the last snapshot in memory; connection failure visibly marks it stale and changes LIVE to OFFLINE. Reloading while offline shows no cached Agent results. Authentication expiration offers reconnect/login rather than presenting the old data as current. Returning from a background tab triggers another observation request.

## Web operations and limits

Web/phone users can inspect agents, model/token values, current tools and commands, evidence, timelines, host processes and semantic tasks. They can search, filter, explicitly bind/unbind an exact ACP/thread to a ChatGPT conversation, explicitly associate a task, and archive/restore an attention item as a Monitor-only acknowledgement. Archive never rewrites source status; archived work stays available under **全部记录** and automatically returns to current attention if that thread later reports newer activity. Those metadata edits are visible in the Windows app after the next collection.

There are **no** web operations for arbitrary commands, file reads, stopping/restarting an Agent, changing Windows startup or exporting the local browser-extension pairing key. Browser control by a development agent means interacting with this implemented web UI, not remotely controlling arbitrary home-computer processes.

Viewing on a phone does not provide new launch-attribution evidence. The original ChatGPT browser extension still observes only its installed browser; unbound historical calls and uninstrumented phone ChatGPT conversations remain unassigned. Existing attribution limitations in `ATTRIBUTION.md` still apply.

No background push notifications, Android/iOS native package, remote wake-on-LAN, multi-machine selector or cloud history replica is included in this release.

## Cloudflare deployment

An existing shared Cloudflare Tunnel may be reused. Only the Agent Monitor ingress rule should be added before its catch-all:

```json
{"hostname":"monitor.example.com","service":"http://127.0.0.1:43218"}
```

Existing unrelated ingress rules and Access applications must be preserved. The Monitor DNS record is a proxied CNAME to the selected tunnel. A dedicated Monitor Access application covers the whole hostname, including the API.

Private `remote.json` contains only the public URL, Cloudflare team domain and this application's audience. It must use Agent Monitor's own Access audience rather than another application's audience. The collector does not need the Cloudflare control-plane API token at runtime.

The deployment helper can prepare the Access application and then publish the route:

```powershell
python scripts/configure-cloudflare.py prepare
python scripts/configure-cloudflare.py publish
```

It reads deployment-specific values from the Windows user environment: `AGENT_MONITOR_PUBLIC_HOST`, `AGENT_MONITOR_CF_ZONE`, `AGENT_MONITOR_OWNER_EMAIL`, `AGENT_MONITOR_CF_TEAM_DOMAIN`, `AGENT_MONITOR_CF_TUNNEL_NAME`, plus `AGENT_MONITOR_CF_API_TOKEN` (or `CLOUDFLARE_API_TOKEN`). It never prints credentials and refuses to overwrite conflicting DNS, an unrelated hostname owner, or unexpected routes. `publish` requires a running, configured origin and an owner policy. A private pre-change ingress snapshot is kept in ignored `.local/`.

Authenticated remote-owner login uses the normal Cloudflare Access browser flow. No bypass or embedded service credential is required by the public source tree. Tests of local real-data web interaction and public anonymous rejection must not be described as proof that an owner has logged in from a real phone.

## Origin authorization and transport

The web server binds only IPv4 loopback. A request counts as local only with an exact loopback Host and no Cloudflare/forwarding headers. A tunnel connection reaching loopback is not by itself trusted. Public Host requests require a valid `Cf-Access-Jwt-Assertion` signed with a key from the configured team's JWKS, with RS256, expected issuer, this app's audience, expiration and not-before validation. The authenticated-email header alone is not accepted as authentication.

POST operations require the same Origin, JSON and an explicit Monitor client header. The remote operation table is explicit; native IPC commands are not dynamically exposed. Bodies are bounded to 16 KiB. API responses use `Cache-Control: no-store`; browser storage never holds the local extension pairing key. Static files are restricted to packaged UI paths.

The UI polls without overlapping requests while visible. A snapshot version uses the collector's existing observation timestamps, including failed-attempt timestamps; unchanged responses have no body. Gzip compresses substantial JSON/text responses. No per-snapshot hash or persistent browser data cache is needed. Actual update latency includes collection time and the next browser poll, not a promised fixed interval.

## Development verification

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm test
npm run desktop:build
# With the updated Monitor running:
node scripts/verify-remote-boundary.cjs
node scripts/verify-web.cjs
```

The web harness uses a real browser and the running Windows collector. It checks desktop/mobile pages, actual source data, a clearly labeled temporary binding with cleanup, theme behavior, refresh, offline behavior, install assets and injected authentication-error recovery. The injected 401 is a failure-handling test, not a substitute for a real Access session.

The boundary harness verifies the actual origin cannot be accessed as public without a signed token, that forwarded requests cannot become local, that native/secret endpoints are absent, that gzip/unchanged delivery works, and that anonymous public HTTPS page/API requests are challenged by Cloudflare. Reports and screenshots remain in ignored `.local/web-qa/` because they may contain private work metadata.

## Maintenance and rollback

Change web assets, Rust behavior and version together, rebuild the NSIS package and install it to avoid serving assets from an old installation. Monitor runtime data survives the upgrade. Do not ship `remote.json`, journals, API credentials or screenshots in the source repository.

To disable external viewing, remove only Monitor's DNS/ingress/Access application or stop Monitor. Never replace the entire shared Tunnel configuration with a reduced template. Keep the original extension bridge local and do not point the public tunnel at port 43217 or AgentDock's command API.
