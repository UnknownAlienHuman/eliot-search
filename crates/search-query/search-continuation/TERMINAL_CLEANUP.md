# Terminal continuation working-set release

Date: 2026-09-22. Tracking: PR #118 / T21.
Base: `62335c0413f7395885baee7ffda737f1ec4cb647`.
Scope: existing canonical `search-continuation`; no daemon cutover.

## Defect and correction

Terminal transitions changed status and returned external cleanup requests,
but retained the entire candidate vector and issued-fingerprint set until
`compact_terminal`. A revoked/expired/completed continuation therefore kept
its query working set while waiting for unrelated external cleanup/compaction.

The common private status transition now drops store-owned candidate storage
and the issued set on every terminal transition. It replaces the candidate
list with `BoundedList::empty()` instead of retaining a cleared vector's capacity.
The payload durability and boot identity remain unchanged.

This covers final ephemeral emission, explicit completion, expiry, scoped
invalidation, old-boot invalidation, and restrictive TTL/window/count/issued-set
limit changes. Partial emission does not call the terminal transition and keeps
its original window and issued-candidate suppression.

Revision validation and complete receipt preparation still precede mutation.
Failed operations do not clear a prefix of the selected records. Emission totals
are prepared before release and remain accurate in the final receipt. Immutable
record fields, token-index entries, denial reasons and exact external cleanup
references remain until caller-authorized compaction. Terminal records still
count toward the existing record quota; dropping their window is not compaction.

## Boundaries

This releases only allocations owned by this in-memory store. It does not revoke
or destroy copies of pages/permits already held by a caller, zeroize freed memory,
guarantee lower process RSS, release an external epoch pin, or delete a durable
checkpoint. Cleanup requests retain their existing meaning and ordering. The
caller must execute them before using the existing `compact_terminal` contract.

A read-only live/expiry rejection does not itself mutate the store. Integration
must invoke the appropriate invalidation/expiry owner and keep its serving
barrier closed when that transition fails. No public API, error code, shared
schema, dependency, persisted format, default limit or workflow is changed.

The inspected daemon path remains `public_runtime_service/kernel/query.rs` ->
legacy `ContinuationCatalog::continue_page`; `CommandState` still has legacy
catalogs and no canonical continuation credential/live-fence input. No fabricated
binding or result fence was added to disguise this remaining integration gap.

## Verification

Eleven new synthetic tests in `src/terminal_cleanup_tests.rs` exercise actual
backing-vector capacity after release, both durabilities, all invalidation
reasons, partial/final acknowledgement, expiry batches, restart selection,
restrictive limits, single/batch revision failures, batch refusal, stable denial,
cleanup references and token quota/compaction. Existing test files are untouched.
These fixtures are not native, CSPRNG, backend or external-cleanup evidence.

The reconstructed 48,507-byte baseline matches Git blob
`f2b05584f998e0ad2209f8773dc83001bc69efe6`. The local tree contains the reviewed
scope, not a full workspace. Byte-preservation and scoped diff checks are not
substitutes for Rust execution.

Required exact-head checks:

```text
cargo +1.98.0 test --locked -p search-continuation --lib
cargo +1.98.0 check --locked -p search-continuation --all-targets
cargo +1.98.0 fmt -p search-continuation -- --check
cargo +1.98.0 clippy --locked -p search-continuation --all-targets -- -D warnings
```

Rust compilation/tests/rustfmt/Clippy: **NOT_RUN**. The actual targeted test
attempt exited 127 (`cargo: command not found`). Direct VM GitHub access also
failed DNS resolution; exact-source reads/writes use the GitHub connector.
No Actions run, independent review, accepted handoff or T21 acceptance is claimed.
