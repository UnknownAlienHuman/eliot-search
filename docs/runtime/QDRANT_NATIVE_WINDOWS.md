# Qdrant on Windows

## Historical 1.19 development artifact

Qdrant 1.19.0 Windows x64 was previously installed and smoke-tested on 2026-09-10:

- build `74f3e85b`, `x86_64-pc-windows-msvc`;
- archive `qdrant-x86_64-pc-windows-msvc.zip`, 29,340,688 bytes,
  SHA-256 `980CB2E1AE771155CF211DA8C0A8A9206B6482BD4EFFDC4DB994D3ADB707B087`;
- executable 84,184,576 bytes,
  SHA-256 `369C562EAE3D89333A13ABFDB522FA209E3F587C1217A1059D817E80814EA9D4`;
- historical ready/create/upsert/search/count/delete smoke succeeded.

This is immutable historical evidence only. It does not establish current availability, Qdrant 1.20 behavior,
product correctness, data compatibility, containment or authentication.

## Selected implementation baseline

R049 selects Qdrant Server/Rust client **1.20.x** for #263/#264 because direct BM25 over a scoring full-text payload
index plus `TextQuery` and the `code` tokenizer remove baseline sparse-vector inference and fixed `avg_len`.

The repository lock still contains client 1.19.0 and no Qdrant 1.20 Windows artifact has been installed or run by
this documentation work. The implementation cut must select exact server bytes and client version together and
record origin, license, hashes and commands.

## Baseline product mode

Use a configured local endpoint:

```toml
[qdrant]
mode = "external"
endpoint = "http://127.0.0.1:6333"
```

Start Qdrant manually, as an installed service or through development tooling. The one bridge:

1. connects with the actual optional API key;
2. observes live server/version/capabilities;
3. creates/verifies the exact physical collection and scoring full-text schema;
4. performs insert-only publication, `TextQuery`, scope/IDF filtering and exact readback;
5. queries the physical collection named by `PublishedView`—never a serving alias.

No executable hash, artifact manifest, owner fence, secret lease or qualification receipt is passed to the bridge.
Release packaging may record exact server bytes independently.

Minimal Qdrant service configuration remains ordinary Qdrant configuration:

```yaml
storage:
  storage_path: ./storage
service:
  host: 127.0.0.1
  http_port: 6333
  grpc_port: 6334
```

Use one actual API key when the selected threat profile requires authentication. Do not wrap it in a lease or
equality protocol.

## Windows evidence required for Qdrant 1.20

Before claiming support, execute on the exact packaged artifact:

- service startup/shutdown and restart against its private data directory;
- scoring full-text index creation with explicit `code` tokenizer/text settings;
- insert-only build, exact readback and `TextQuery`;
- payload scope plus independent `idf.corpus` behavior;
- process crash/restart with durable current generation verification;
- complete next generation and old pinned view;
- terminal collection deletion and exact logical absence;
- delayed request versus retired-collection delete;
- bounded memory/disk/handles, current + building + retained generations;
- authentication, cancellation and content-free diagnostics.

Historical 1.19 smoke does not substitute for any of these.

## Optional managed-child mode

A later convenience profile may spawn the exact configured server and retain the actual child handle. It must use
private create-new resources, sanitized environment, bridge-authenticated readiness, one absolute deadline and an
actual Windows Job Object/kill-on-close tree guard or report managed mode unsupported. Failure/shutdown performs
bounded terminate and reap. No attach by PID/name/port, automatic restart, auto-download or qualification platform.

The current `search-qdrant-supervisor` does not satisfy this boundary and must be replaced or deleted under #334
after current callers are rechecked.

## Qdrant Edge is separate

Qdrant Edge is an embedded backend replacement experiment, not a managed server. The official 0.8.0 line remains
beta and recently exposed WAL acknowledgement/replay defects. Evaluate only a newer released line after the server
walking skeleton works; require Windows persistence/crash, BM25/filter, generation lifecycle, build-size and
latency evidence. Never ship server and Edge as permanent dual backends.

## Updating Qdrant

Updates are explicit. Install/test a new server/client/profile side by side, build a fresh physical generation
when schema/retrieval meaning changes, commit matching evidence/current state and install the view only after exact
readback. Do not auto-upgrade or silently reuse an incompatible data directory.
