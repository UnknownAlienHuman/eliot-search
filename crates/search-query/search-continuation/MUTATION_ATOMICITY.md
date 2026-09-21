# Canonical continuation mutation atomicity

Date: 2026-09-21. Tracking: PR #118 / T21.
Base: `249934e3af88e5f3ff5bec98d8e425735e7b5119`.
Scope: the existing `search-continuation` owner; no daemon cutover.

## Defect and correction

`commit_emission` extended the issued set before checking the next record
revision. `complete` changed status before the same check. Expiry, invalidation
(including restart invalidation), and live-limit application performed that
check inside a mutating loop. A late `RevisionExhausted` could therefore leave
changed records, lose the caller's cleanup effect list, and leave new limits
unpublished after a partial transition. These paths had no rollback.

The corrected ordering is:

1. Select exactly the same records, preserving scope, expiry order, batch bounds,
   generation conflicts and already-terminal/idempotent exclusions.
2. Validate every selected next revision and prepare the complete bounded
   identity/effect lists before changing any record.
3. Apply the prepared revisions/statuses under the same exclusive store borrow.
   Publish new limits only after this non-fallible transition phase.
4. Return the prepared receipt without another fallible conversion.

Single-record emission/completion similarly prepare the next revision and
cleanup result before changing issued state or terminal status. Compaction
constructs its bounded returned identity list before removing records and token
index entries. No candidate window or issued set is cloned for batch staging;
only selected IDs, next revisions and cleanup references are retained.

## Preserved boundaries

Public signatures, shared record shapes, error codes, quota selection policy,
token lookup, source/security revalidation, and durable/ephemeral effect kinds
are unchanged. There is no new dependency, persistent format, state owner,
workflow or qualification identity.

Atomicity here concerns **returned Rust errors within the in-memory owner**.
It does not cover process death, allocator panic, external pin release,
durable-checkpoint deletion, or bytes already delivered to a client. Receipts
request cleanup; they do not prove cleanup was executed. On failure, integration
must keep the relevant deny/restart/configuration barrier closed and must not
interpret the unchanged records as authorization to resume serving.

The revision-exhaustion cases use private fault injection. They establish a
boundary condition without claiming a production workload reached 2^64 updates.
This patch does not wire the legacy daemon catalog to this canonical owner,
complete T20/T21, bind an emission permit to its exact selected candidate set,
or add the remaining cancellation and emission-time live-fence integration.

## Regression source

`src/atomicity_tests.rs` adds 16 deterministic tests. Fixtures are synthetic
in-memory records and references, not CSPRNG, native pin, storage or live-provider
evidence. Complete state snapshots include all records, payloads, issued sets,
revisions, terminal reasons, invalidation generations, limits and token indices.

The cases cover both durabilities; emission/completion exhaustion; first/middle/
last-record batch faults; restart; failed live-limit changes; exact u64 boundary
success; normal partial/final emission and stale permits; expiry ordering and
bounded sweeps; exhaustion outside the selected batch; exact cleanup association;
idempotent invalidation; generation/batch refusal; and safe token-index compaction.

## Verification

The reconstructed original file matched Git blob
`7d828ffe2aa31c4e06cc5cc659bbd767cf1c5574` (47,544 bytes). The local tree is
scope-only. Source checks must not be represented as a workspace build.

Required exact-head execution:

```text
cargo +1.98.0 test --locked -p search-continuation --lib atomicity_tests
cargo +1.98.0 test --locked -p search-continuation --all-targets
cargo +1.98.0 check --locked -p search-continuation --all-targets
cargo +1.98.0 fmt -p search-continuation -- --check
cargo +1.98.0 clippy --locked -p search-continuation --all-targets -- -D warnings
```

Rust compile/tests/rustfmt/Clippy: **NOT_RUN**. Cargo is absent in this
environment; the attempted test command exits 127. No Actions run was started.
No independent review, gate, accepted handoff or product readiness is claimed.
