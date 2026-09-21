# DIRECT continuation: retain one ranked window during paging

Date: 2026-09-21. Base: `28a869418a360957bf942e9d9b65f72f42c304d9`.
Tracking: T21 / PR #118; session-state work tracked by PR #193.

## Corrected implementation

`ContinuationCatalog::continue_page_with_live_barrier` cloned an entire
`ContinuationRecord` after token lookup, removed the original record, copied the
selected page, and cloned the record again when reinserting a nonterminal window.
Each record contains the complete retained `Vec<StoredMatch>` and its owned
strings. The configured retained-match counter did not account for these
transient duplicate windows.

Validation now borrows the existing record. After the same validation sequence,
a private `advance_window` operation updates its cursor in place and clones only
the selected page and fixed-size coverage. Nonterminal pages neither remove nor
reinsert the record. Terminal pages use the existing `drop_window` accounting;
the existing expiry sweep still runs after successful advancement.

`ContinuationRecord` no longer implements `Clone`. This prevents restoring the
former whole-record copies without explicitly changing the ownership boundary.
No first-page behavior, token format, quota, TTL, source-fence digest, public API,
persisted format, dependency, workflow, or configuration was changed.

## Preserved validation and behavior

The externally reachable method still checks, in the same order:

1. page-size limits;
2. session entropy availability;
3. token/session identity;
4. live purge, revocation and generation barriers;
5. record expiration;
6. exact namespace and source fence.

Only then can the private paging helper run. The helper is not an authorization
API. Existing denial paths still discard the whole affected ranking. Paging
keeps the original token, deadline, immutable coverage and source binding; it
does not renew TTL, refresh against a newer corpus, repeat gap details, or report
truncated coverage as complete. The retained-match quota continues to count the
whole retained vector until its window is removed, including already emitted
matches, exactly as before.

## Regression source

Eight new package-local unit tests in `continuation/kernel/catalog/tests.rs`
cover retained allocation/capacity identity, complete multi-page order, stable
coverage/TTL/binding, independent-window accounting, unknown-token no-op,
expired-other-window cleanup, incomplete coverage, explicit invalidation and an
empty terminal window. These use synthetic in-memory records to exercise the
actual private paging helper; they do not qualify entropy, authentication or
source provenance. Every pre-existing continuation test remains unchanged.

## Verification boundary

The original catalog was reconstructed byte-for-byte and matched Git blob
`026b1dd985d2766b80ad7f0e2ad34ff7ff799b51`. Scoped whitespace, source-preservation,
validation-order and copy-site checks were performed locally. The local Git tree
contains only this reviewed scope, not the full repository.

Rust compilation, unit/process tests, rustfmt and Clippy: **NOT_RUN**. Cargo is
absent; the actual test command exits 127. Direct VM downloads also fail DNS;
GitHub connector access is separate. No workflow was started.

Required execution on the pinned toolchain:

```text
cargo +1.98.0 test --locked -p eliot-searchd continuation::kernel
cargo +1.98.0 check --locked -p eliot-searchd --all-targets --all-features
cargo +1.98.0 clippy --locked -p eliot-searchd --all-targets --all-features -- -D warnings
cargo +1.98.0 fmt --all -- --check
```

No elapsed-time, RSS or product-performance improvement is asserted without
measurement. The demonstrated source change removes whole-window clone sites;
it does not make source-fence validation or the complete request constant-time.

## Remaining T21 boundary

This is a correction to the existing legacy DIRECT path, not canonical T21
acceptance. The canonical `search-continuation` API already requires shared
binding/grant/result-fence records, token digests and pin/replan contracts. The
legacy daemon catalog must be replaced through that accepted API, not copied
into the package as another competing store. No new compatibility catalog,
accepted handoff, gate, launch state, or independent review was created here.
