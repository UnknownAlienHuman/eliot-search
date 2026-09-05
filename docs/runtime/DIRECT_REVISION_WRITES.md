# DIRECT revision write ordering

The primary Windows DIRECT store supplies a protected revision writer to the source catalog:

```text
bounded same-handle source snapshot
  -> exact source/revision/SHA-256/length binding
  -> DPAPI object persistence
  -> decrypt and exact byte readback
  -> source-event publication
```

This protected path creates neither a new `.bin` revision nor a plaintext temporary file. The
plaintext-development writer remains explicit on other platforms. Referenced legacy plaintext is
migrated separately on opening: protect and verify, then remove the old directory entry. That removal
is not physical secure erasure and does not erase previous backups or filesystem history.

## Immutable object publication

Only protected bytes enter the staging file. After flushing it, publication uses a no-clobber hard link,
not a rename that can replace an existing destination. Conflicting existing objects, links and invalid
object types are refused. Filesystems without hard-link support fail closed; there is no overwrite
fallback. Failed writes close the staging file before cleanup, including on Windows. An exact encoded
readback precedes success; the revision writer additionally decrypts and compares the exact input bytes.

A retry checks existing ciphertext by decryption. It does not require two randomized DPAPI calls to
produce identical ciphertext. A new admission refuses a same-ID orphan `.bin` with
`DIRECT_NEW_REVISION_PLAINTEXT_PRESENT` instead of silently retaining it beside ciphertext. Referenced
legacy migration uses a separate entrypoint and remains supported. Orphan cleanup is explicit; this
refusal does not claim that old plaintext elsewhere has been removed.

## Batch and retry semantics

Input snapshots and duplicate identities are checked before storage. A batch retains at most 512 MiB
of source bytes, with the remaining budget applied before each allocation and a 64 MiB per-file cap.
Empty files consume zero source bytes. These limits do not describe total process RSS or latency.

Every revision writer must succeed before any new source event in that batch is appended. Earlier
object writes may leave orphan ciphertext when a later writer fails, but no batch metadata is published
at that point. Even an unchanged source requires storage readback. Stale in-memory catalogs are rejected
before ingestion effects.

Index-operation v2 binds the previous source record. A -> B -> A and retired-source reactivation are new
transitions, not replays of the first admission. Source/revision IDs and existing event records stay
unchanged. Repeating the already-current active revision/path is a no-op after storage readback.

## Remaining boundaries

The append-only source log is not yet an atomic multi-record database. An interrupted log append or
post-append readback still needs repair/recovery; daemon control-state migration to redb is unfinished.
The object publication barrier does not imply rollback of an ambiguous log commit or atomicity across
registered roots.

Native final-handle/race admission, Windows power-loss durability, complete old orphan cleanup, durable
representation/unit manifests and live Qdrant publication remain unqualified or unfinished. The process
exit test below is not a machine power-loss test. No new cipher, dependency, Python service or index is
introduced.

## Verification

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --locked -p eliot-searchd --bin eliot-searchd
```

Ingestion tests cover writer failures, byte budgets, A/B/A, reactivation, stale catalogs and empty files.
Filesystem tests cover identical retries, conflicting and racing writes, empty objects and Unix symlinks.
Their synthetic encoded bytes test publication only, not cryptography.

Windows-only tests use the real platform protector for file/batch rejection, corrupt ciphertext, legacy
migration, orphan-plaintext refusal and process exit after ciphertext readback but before the source
log append. The process test reopens the data root, verifies that no source event or plaintext copy was
published, then retries against the existing encrypted orphan. Its ignored helper is launched by the
parent test; no failure-injection switch is added to the product executable.

The existing manual workflow's optional `core_tests` input runs these tests on the selected platform.
Use `windows-2025` for DPAPI coverage. The default remains one workspace check; the trigger remains
`workflow_dispatch` only. Compilation, Rust tests, formatting and Clippy have not been executed in the
authoring environment. Passing-build and native-security claims require those runs, not source presence.
