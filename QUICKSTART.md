# ELIOT Search — current development quickstart

The current daemon captures bounded immutable snapshots of admitted UTF-8 files,
stores exact retained revisions under the data root, and serves DIRECT queries
from those retained revisions rather than reopening live source paths.

The storage profile is still developmental: retained revisions are plaintext and
the fingerprint is collision-checked but not cryptographic. Responses therefore
report `source_backed=true`, `encrypted_at_rest=false`, and
`production_ready=false`.

## Build

```powershell
cargo build --locked -p eliot-searchd -p eliot-search
```

On Linux, omit `.exe` in the commands below.

## Start the daemon

```powershell
./target/debug/eliot-searchd.exe serve `
  --data-root ./.eliot-search `
  --source-root .
```

Startup now performs this sequence before publishing the endpoint:

```text
exclusive OS file lock on the data root
→ bounded same-handle reads of admitted files
→ exact retained revision write + readback
→ immutable snapshot manifest write + readback
→ crash-tolerant control-state publication
→ loopback endpoint publication
```

The default capture ceilings are 10,000 files, 2 MiB per file, and 512 MiB for
the complete snapshot. Sensitive, binary, generated, vendor, cache, VCS, and
credential-like paths are excluded by the built-in development policy.

## Query the retained snapshot

In another terminal:

```powershell
./target/debug/eliot-search.exe health --data-root ./.eliot-search
./target/debug/eliot-search.exe status --data-root ./.eliot-search
./target/debug/eliot-search.exe search "source" --data-root ./.eliot-search
```

Each result includes:

- frozen snapshot identity and manifest fingerprint;
- exact retained revision fingerprint;
- root-relative path;
- line, byte-column, absolute byte start, and absolute byte end;
- bounded excerpt;
- explicit denominator, unavailable-revision, truncation, and completeness fields.

The current query policy checks an exact match first and an ASCII-insensitive
match second. It does not claim stemming, fuzzy matching, semantic search, or
complete Unicode case folding.

## Refresh after source changes

```powershell
./target/debug/eliot-search.exe refresh --data-root ./.eliot-search
```

Refresh builds another immutable snapshot and changes the active in-memory view
only after the new manifest and control state have been written and read back.
Existing retained revisions are reused only after exact byte and fingerprint
verification.

## Stop cleanly

```powershell
./target/debug/eliot-search.exe shutdown --data-root ./.eliot-search
```

Shutdown records `STOPPED`, removes the endpoint descriptor, writes the released
owner state, and releases the OS file lock.

## Current truth boundary

Implemented on `main`:

- loopback-only authenticated local protocol and CLI;
- live-process OS file lock instead of stale `create_new` ownership;
- alternating crash-tolerant lifecycle/control-state files;
- bounded source inventory with deny rules and explicit gaps;
- same-open-handle metadata verification before and after source reads;
- immutable retained UTF-8 revision objects with exact readback;
- frozen snapshot manifests with deterministic ordering;
- DIRECT search over retained revisions, not current source paths;
- explicit refresh, status, health, and clean shutdown.

Still required for the production source profile:

- cryptographic content digests and encrypted revision storage;
- hardened Windows final-handle containment and ACL verification;
- OS secret-store integration for the endpoint token and encryption keys;
- complete admission/identity/registry/materializer/unitizer receipts wired into
  daemon composition;
- durable migration/recovery integration and production qualification evidence;
- lexical sparse indexing and Qdrant publication for scalable retrieval.
