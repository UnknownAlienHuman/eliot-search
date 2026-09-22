# DIRECT page preparation and delivery

Date: 2026-09-22. Tracking: PR #118 / T21.
Base: `f804716e90b420e1248ceb79347063652ff02ae5`.
Scope: the existing daemon DIRECT command path. No canonical-owner cutover.

## Defect in the serving path

`cmd_continue` advanced the continuation cursor (or removed a final window)
before `mint_page` and `refresh_storage`. A handle-capacity, source-validation,
entropy or diagnostic failure could return an error without any page, yet a
retry would start after the undelivered results. `cmd_search_page` could retain
a newly minted continuation whose token was never returned. Successfully minted
handles could likewise remain unpublished after a later preparation error.

Both commands now use the same sequence:

```text
validate existing admission/session/source conditions
-> prepare page under exclusive continuation-catalog borrow
-> prepare opaque handles under exclusive handle-catalog borrow
-> refresh storage diagnostics
-> check original page and handle deadlines
-> write the complete page exchange
-> commit continuation cursor/new window and prepared handles
```

New windows and handles are not inserted during preparation. Dropping a
preparation releases only its unpublished values. Existing live rankings remain
in place, with their original cursor, deadline and quota charge. This includes
would-be final pages. Only the selected page is copied; no full-ranking clone or
parallel catalog is added. Exclusive borrows prevent intervening catalog writes
or quota consumption before commit.

A completed output callback commits the cursor/new window. Handle insertion
then has no recoverable validation step. There is no post-output TTL check that
could replace an already-written complete page with an error. Before starting
output, late expiry discards the affected continuation or unpublished handle
batch. Reported TTL is recomputed from the original deadlines, never renewed.
Already-expired records may still be swept by existing housekeeping; this is not
a promise that an aborted command preserves unrelated expired records.

## Output failure and security limits

`SessionOutput` now remembers whether the current command wrote any bytes.
`session::serve` fails stop on a command error after output starts, even when
all writes themselves succeeded. For example, a later frame can exceed its
size bound after a valid `search_page_started` frame; that logical error must
not append an error frame and continue to the next command. Existing I/O,
nonempty zero-write and flush failures remain fatal. Mutation attempts retain
`SERVICE_MUTATION_OUTCOME_UNKNOWN`; other incomplete exchanges return the
existing `SERVICE_OUTPUT_FAILED`. Errors before output may still return their
bounded error frame and continue. The flag resets for each new command.
The existing runtime invalidates both catalogs on session failure. Preserving
an uncommitted cursor is **not** permission to replay partially delivered bytes
in the same session.

Complete writer success is the acknowledgement boundary used here, not proof
that a remote client consumed bytes. This patch does not provide crash/panic or
allocator-failure rollback, exactly-once network delivery, cancellation of a
blocked writer, or continuous TTL/security enforcement while output is in
progress. The final deadline observations precede output; slow output can cross
a deadline and subsequent requests must still revalidate it.

The original legacy session/source-fence checks and their clean-barrier call
path are retained, not upgraded into canonical client/grant authorization.
Connecting `search-continuation`, qualified pins/credentials, live security and
the actual output barrier remains T20/T21 work. No fake grant, pin, receipt or
always-successful canonical adapter was introduced.

## Handle token reservation

The old mint loop checked generated tokens against committed handles only;
two repeated entropy values in the same staged batch could select the same
key and overwrite one record on insertion. The mint loop now reserves each
staged token too. Both committed and reserved collisions consume the existing
128-attempt per-token budget. Exhaustion/entropy failure inserts no batch prefix.
Production entropy and token encoding are unchanged; the injection seam is
private and used for deterministic collision tests only.

## Regression source and verification boundary

Twenty-two new unit tests are in the two `catalog/delivery_tests.rs` modules
and `public_runtime_service/kernel/session/delivery_tests.rs`:
10 continuation tests, 7 handle tests and 5 session-loop tests. They cover
dropped preparation,
pre-output refusal and exact retry, final pages, unpublished first-page cleanup,
original deadlines, expired housekeeping, partial output failure, no post-output
error, page ordering, handle publication, empty batches, committed/staged token
collisions, retry exhaustion and entropy failure. The session tests exercise
the real `serve` function with in-memory I/O: logical failure after output,
mutation-unknown classification, refusal before output, per-command flag reset,
and empty writes/flushes. Fixtures are not native filesystem, authentication
or external transport proof.

Existing catalog, handle, expiry and fail-stop test files are unchanged. The
old immediate catalog methods remain test-only compatibility seams; production
`cmd_search_page` and `cmd_continue` use preparation and complete-output commit.

The four reconstructed baseline files matched their Git blob identities:
`2abed32aaaef97ceef0bf679603bc2d19ec5486f`,
`486c996ab9672c93de2446f21971b4c7d2466c7a`,
`64cf2dc1bb184aceb5cefad4f51fde3483740726`, and
`2f998a1c4479622524396c814249d717a732ace6`.
Scoped diff, source-preservation, module routing and finite token-reservation
checks were performed. These are not compiler or unit-test execution results.

Required exact-head execution includes:

```text
cargo +1.98.0 test --locked -p eliot-searchd --all-targets delivery_tests
cargo +1.98.0 test --locked -p eliot-searchd --all-targets continuation::kernel
cargo +1.98.0 test --locked -p eliot-searchd --all-targets result_handles::kernel
cargo +1.98.0 test --locked -p eliot-searchd --all-targets session::tests
cargo +1.98.0 test --locked -p eliot-searchd --test live_handles_process
cargo +1.98.0 check --locked -p eliot-searchd --all-targets --all-features
cargo +1.98.0 fmt -p eliot-searchd -- --check
cargo +1.98.0 clippy --locked -p eliot-searchd --all-targets --all-features -- -D warnings
```

Rust compile/tests/rustfmt/Clippy: **NOT_RUN**. The actual targeted test command
exited 127 (`cargo: command not found`). No local compiler/cache was found;
the official manifest download route was unavailable and audited instead of
repeated. The local Git tree is a scope-only reconstruction, not the workspace.
No Actions run, independent review, accepted handoff or T21/product acceptance
is claimed. Shared APIs, dependencies, lockfile, token/wire formats, defaults,
qualification identities and workflow files are unchanged.
