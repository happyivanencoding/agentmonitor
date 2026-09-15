# Onward recruitment migration — 0.5.8

Version 0.5.8 retires the Onward recruitment dashboard from Agent Monitor. The React page, native `get_onward_jobs`, Rust job-data reader and `/api/onward-jobs` endpoint are removed. The standalone Onward Control Jobs module now owns that operating interface; local collection, SQLite authorities and sync tasks are unchanged.

Agent Monitor returns to Runtime/Agent/process/task observability. It does not aggregate a market-data platform as an agent. Onward Control does not depend on Agent Monitor being open. See [migration notes](docs/ONWARD_CONTROL_MIGRATION.md). The actual private Control URL remains in deployment configuration; an illustrative URL is `https://projectos.example.com/onward/`.

Build validation: TypeScript/Vite, optimized Rust/NSIS and existing Node tests passed. The installed application was updated to 0.5.8. Jobs ownership moved; no Raw/Derived data was deleted and no collector, worker or ACP session was restarted for the migration.

Do not recreate a recruitment tab here or add a compatibility HTTP reader. Use the separate Control service and its own observer. Agent Monitor history may still mention the retired integration; those notes are not current architecture.
