# DIRECT output deadline enforcement

Date: 2026-09-22. Tracking: PR #118 / T21.
Base: `1c85492d5f55cddfa4f90a2b7118dc8cdd8044ec`.
This extends `DIRECT_PAGE_DELIVERY.md`; canonical owner cutover remains open.

## Corrected runtime paths

Both paged-search commands now carry the original absolute continuation and
handle deadlines into one private `output_deadline` adapter. The earlier one
wins. A final continuation page still has its original deadline even though it
returns no next token. An empty handle batch introduces no artificial lifetime.
The deadline is never reconstructed from a rounded `expires_in_ms` value.

The adapter checks `Instant::now()` before every `write` and `flush`, including
partial-write retries, newlines, and later response frames. Once expiry is
observed it is latched: no further inner operation is started, even if an
emitter catches the error. The existing typed expiry code is returned before
any output; the existing session loop fails stop after partial output without
appending another error frame or consuming a queued command. Prepared pages and
handle batches commit only after the complete output callback succeeds.

Handle expansion also retains its catalog borrow and original deadline through
caller-side diagnostics and output. All prior session/barrier/namespace/source/
revision/range/readback checks remain in the same order. Successful readback is
followed by a deadline/presence check: bytes from a handle that expired or was
swept during reading are not returned. Another check precedes the output callback;
the same output adapter then checks each write/flush. Preparation is not a new
catalog, does not renew the handle, and is not clonable. Existing immediate
expansion methods remain test-only compatibility seams.

## Limits of the guarantee

An already-running blocking `Write` or `flush` cannot be interrupted by this
adapter. Bytes accepted before an expiry observation cannot be retracted. A
successful final operation is not retroactively changed to an error; there is
no fallible post-output TTL check before page/handle publication. Transport
cancellation/timeouts, remote receipt, crash/panic rollback and exactly-once
semantics are not established here.

This does not add canonical client/grant authorization or observe external
revocation during output. Existing legacy source/session/clean-barrier semantics
remain unchanged. Canonical credentials, live security-barrier wiring, native
pins and qualified cancellation are still required for T21 acceptance. It is
not secure erasure of discarded source bytes or an external cleanup receipt.

The change stays inside the daemon package: no dependency, shared schema,
wire field, token format, reason code, default limit or workflow change. The
unpaged search path and existing serializers/session implementation are unchanged.

## Evidence

Ten new output tests cover the earliest deadline, exact-boundary refusal,
short writes, inter-frame expiry, flush refusal, successful final flush,
latched expiry, unchanged non-expiry errors, and the actual session loop with
in-memory I/O. Five synthetic expansion tests cover read-completion expiry,
already-swept records, caller-side delay, successful output and abandoned/error
preparations. Existing test files are unchanged. These fixtures do not execute
native storage/transport or prove canonical integration.

Five baseline files were reconstructed at their exact Git blob hashes.
Scoped `git diff --check` and source-preservation/routing checks passed. The
local tree contains only the reviewed scope, not a full workspace checkout.

`cargo +1.98.0 check --locked -p eliot-searchd --all-targets` exited 127:
`cargo: command not found`. Rust compilation, unit/process tests, rustfmt and
Clippy are **NOT_RUN**. No Actions run or acceptance receipt was created.
Required exact-head follow-up includes that check, `cargo +1.98.0 test --locked
-p eliot-searchd --all-targets`, package rustfmt and strict Clippy. Source test
counts are not passing execution evidence; independent review remains required.
