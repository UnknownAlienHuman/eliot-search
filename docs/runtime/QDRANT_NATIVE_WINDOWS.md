# Qdrant on Windows

Qdrant 1.19.0 Windows x64 was previously installed and smoke-tested as recorded below. Those measurements are
historical development/release evidence; they are not runtime credentials and do not require the application
to preserve the old qualification supervisor.

## Recorded development artifact — 2026-09-10

- version/build: `1.19.0` / `74f3e85b`, `x86_64-pc-windows-msvc`;
- archive: `qdrant-x86_64-pc-windows-msvc.zip`, 29,340,688 bytes,
  SHA-256 `980CB2E1AE771155CF211DA8C0A8A9206B6482BD4EFFDC4DB994D3ADB707B087`;
- executable: `qdrant.exe`, 84,184,576 bytes,
  SHA-256 `369C562EAE3D89333A13ABFDB522FA209E3F587C1217A1059D817E80814EA9D4`;
- license: Apache-2.0;
- recorded smoke: ready/create/upsert/search/count/delete succeeded.

This does not establish current availability, data compatibility, containment, authentication or product
correctness. Check the service actually selected for the current run.

## Baseline product mode

The first connected product uses a configured local endpoint:

```toml
[qdrant]
mode = "external"
endpoint = "http://127.0.0.1:6333"
```

Start Qdrant manually, as an installed service or by development tooling. The daemon's one Qdrant bridge:

1. connects with the selected optional API key;
2. observes the live server/version/capabilities;
3. verifies the exact physical collection generation/schema it will use;
4. performs publication/query/readback.

No executable hash, artifact manifest, probe report, owner fence, secret lease or process receipt is passed to
the bridge. Release packaging may record exact server bytes independently.

Minimal local Qdrant configuration remains ordinary Qdrant configuration, for example:

```yaml
storage:
  storage_path: ./storage
service:
  host: 127.0.0.1
  http_port: 6333
  grpc_port: 6334
```

Enable Qdrant authentication when the selected local threat/profile requires it. One actual API key is used by
the service and bridge; the baseline does not build a lease/equality protocol around it.

## Optional managed-child mode

A later standalone profile may let the daemon spawn the executable. It is a convenience, not a prerequisite
for indexed search.

The implementation must be small and concrete:

- spawn only the configured executable and hold the actual child handle;
- never attach to an existing PID/process name/port;
- use private create-new config/resources and a sanitized environment;
- use the real Qdrant bridge/client for authenticated readiness;
- use one absolute startup/shutdown deadline;
- apply an actual Windows Job Object/child-tree guard or report managed mode unsupported;
- bounded terminate/reap on failure and shutdown;
- no automatic restart in the first profile;
- no fixed unbounded `run-stdout.log` / `run-stderr.log` in the index data directory.

The current `search-qdrant-supervisor` does not satisfy that boundary: it has no product caller, cannot call
Windows Job/ACL APIs, accepts caller-created containment evidence and surrounds child spawning with roughly
90.7 KiB of qualification/receipt state. Replace or delete it under #334.

## Updating Qdrant

Updates are explicit. Install/test a new server/client/profile side by side, create a fresh collection
generation when schema/retrieval meaning changes, run the connected product checks and switch the published
view only after exact readback. Do not auto-download, auto-upgrade or silently reuse an incompatible data
directory.

Exact artifact checks and broader installed/fault/resource qualification belong to release work after the
product path exists. They do not block direct endpoint development or become constructor tokens.
