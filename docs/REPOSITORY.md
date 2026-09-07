# Repository and publication policy

Canonical repository:

`https://github.com/happyivanencoding/agentmonitor`

## Required workflow

The local project root remains `C:\dev\agent-monitor`, but `main` is expected to stay synchronized with `origin/main`.

For every future source-code change to Agent Monitor:

1. update the relevant handoff/docs when behavior or operating assumptions change;
2. commit the completed code change to the local `main` branch;
3. push that commit to `origin/main` in the same development task;
4. do not report the code change as fully delivered if the requested push failed.

Documentation-only edits that are part of a code change should be included in the same commit. Pure temporary diagnostics, screenshots and runtime observations must remain local.

## Privacy boundary

This repository is intended to contain portable source code and public-safe documentation only. Never commit:

- real user email addresses, personal names or user-profile-specific paths;
- API tokens, Cloudflare credentials, browser-extension pairing keys, OAuth material or DPAPI blobs;
- `%LOCALAPPDATA%\AgentMonitor` runtime data, SQLite journals, `remote.json`, deployment IDs, screenshots or diagnostic captures;
- full ChatGPT transcripts, private prompts captured from real use, tool outputs or business-project source files;
- deployment-specific hostnames or account identifiers when an example/environment variable can be used instead.

Use `%USERPROFILE%`, `%LOCALAPPDATA%`, `monitor.example.com` and other explicit placeholders in documentation. Cloudflare deployment-specific values belong in local environment variables and ignored runtime files.

`.gitignore` is part of this boundary, but it is not a substitute for reviewing the actual staged diff before publishing.

## Git identity and history

Use a non-personal Git author identity for this repository. Do not publish earlier local history that contains machine-specific or deployment-specific details merely to preserve commit chronology. The public remote should contain only the sanitized publication history.
