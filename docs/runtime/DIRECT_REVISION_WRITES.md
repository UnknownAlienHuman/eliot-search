# DIRECT revision write ordering

The primary Windows DIRECT store now supplies a protected revision writer to the existing source
catalog. New revision bytes follow this order:

```text
bounded same-handle source snapshot
  -> exact source/revision/SHA-256/length binding
  -> DPAPI object persistence
  -> decrypt and exact byte readback
  -> source-event publication
```

Neither a new `.bin` revision nor a plaintext temporary file is created by this protected write path.
The existing plaintext-development writer remains explicit on other platforms. Existing referenced
plaintext objects are still migrated on opening: protect and verify first, then remove the old object.
Removing a directory entry is not secure physical erasure.

## Batch and retry semantics

All input snapshots and duplicate source identities are checked before invoking storage. The complete
batch has a 512 MiB retained source-byte ceiling; each read receives the remaining budget before
allocating its buffer. The existing 64 MiB per-file ceiling also applies. Empty files consume zero
source bytes. These are source-buffer limits, not a total-process RSS or latency guarantee.

Every revision writer must succeed before any new source event in that ingestion batch is appended.
Failure before that point leaves the source catalog unchanged but can leave orphan revision objects.
A retry reads and verifies an existing object rather than overwriting it. Even an unchanged source
requires storage readback. Native Windows tests deliberately block a new protected target to check
that neither new metadata nor a plaintext copy is published on protection failure.

Index-operation v2 identities additionally bind the exact previous source record. Returning from A to
B to A, or reactivating a retired source with the same bytes, is therefore a new transition rather than
a replay of its first indexing operation. Retained source/revision IDs and existing event-log records
remain unchanged. Repeating the already-current active revision/path remains a no-op after readback.
A stale in-memory catalog is rejected before new ingestion storage effects.

## Remaining boundaries

This change does not make the append-only source log an atomic multi-record database. An interrupted
log append or post-append readback still needs the existing repair/recovery path; the daemon migration
to the real redb adapter is not complete. The prepublication guarantee is about revision storage
failure before log append, not rollback of an ambiguous log commit or atomicity across roots.

Full native file-handle/race admission, Windows power-loss durability, old orphan-plaintext cleanup,
canonical durable representation/unit manifests, and live Qdrant publication remain unqualified or
unfinished. No fallback cipher, new dependency, Python service or alternative index is introduced.

## Verification

Seven ingestion unit tests cover rejected writers, a later writer failure, aggregate byte budgets,
A/B/A transitions, reactivation, stale catalogs and empty files. Two Windows-only primary-store tests
use the real platform protector and fail its next target for a file and a directory batch. They assert
byte-identical old control state, no new `.bin`, and successful exact retry after the failure is removed.

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --locked -p eliot-searchd --bin eliot-searchd
```

The manual workspace workflow now has an optional `core_tests` input. On `windows-2025` it includes
these native tests as well as control, preparation and primary-process regressions. Its default still
runs one workspace check, and all triggers remain `workflow_dispatch` only.

Compilation and Rust test execution were unavailable in the authoring environment. These checks are
required before claiming a passing build or Windows security qualification; added tests are not executed
evidence. No product or gate acceptance is issued by this change.
