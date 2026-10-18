# Runtime family rules

Read the root [architecture](../../docs/architecture/README.md) and
[implementation](../../docs/implementation/README.md).

- `search-runtime-owner` owns only the process/data-root owner epoch and concrete acquire/drain/release.
- `search-os-secrets` owns optional OS-user-bound secret references; plaintext stays out of public config,
  argv, logs and telemetry.
- Published-view lifetime belongs to publication/query owners, not a generic runtime lease catalogue.
- Ordinary Qdrant cleanup belongs to publication plus the private bridge delete/readback operation.
- `search-retention` is a removal target. Do not extend its lease, purge, sweep, restore or migration APIs.
- The daemon is the composition root; this directory is not a Cargo package.

A runtime package exists only for a real OS/process dependency or failure state. Do not create a package,
port, receipt or state machine for hypothetical legal hold, migration, remote ownership or future storage.
