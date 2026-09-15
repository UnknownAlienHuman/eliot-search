# Indexed-query CLI and qualification gate increment

Date: 2026-09-15. Base: `9418acf46bebf755f7df2e2a2261cbcc2b683b87`.
Tracking: product-test packet PR #185; T19/T28/T30 integration path.

## Result

The public CLI now has an explicit command:

```text
eliot-search indexed "TERMS" [--data-root DIR] [--address IP:PORT] [--token-file PATH]
```

Indexed intent is encoded with one versioned, NUL-delimited marker owned by
`search-provider-protocol`. The marker is recognized before generic query
capability admission. Ordinary DIRECT query text cannot accidentally select the
indexed path, and a recognized marker with an empty query fails with
`PROVIDER_INDEXED_QUERY_INVALID` rather than falling back.

The daemon applies a separate indexed gate requiring both:

- general accepted query readiness; and
- indexed/Qdrant qualification and route readiness.

On the current repository state, `qualification/qdrant/artifact.toml` remains
`UNQUALIFIED`, and the daemon uses default empty accepted receipts. Therefore
the command returns:

```text
PROVIDER_INDEXED_QUERY_UNAVAILABLE
INDEXED_NOT_ACCEPTED
```

with process exit code 2. It cannot return an empty success or silently execute
DIRECT search. A synthetic inconsistent capability snapshot also fails closed.

## Structure

The mature CLI application and pairing/proof transport were moved byte-for-byte
to internal `core.rs` modules and are reused by thin facades. No alternate
transport, token derivation, endpoint discovery, proof verification or provider
session was introduced. The daemon addition recognizes and gates mode only; it
constructs no Qdrant client and owns no search state.

New regression coverage includes:

- protocol marker round-trip, ordinary-query separation and empty-marker reject;
- CLI facade framing through the existing query encoder;
- strict indexed option parsing and loopback-only addressing;
- daemon mode classification plus accepted/denied/inconsistent capability cases;
- real daemon + real CLI process path proving typed unavailability, blocker
  disclosure and absence of `status=ok`.

## Explicit remaining work

This increment closes the missing public verb and unqualified fail-closed path.
It does **not** claim a live indexed query: after an accepted qualification gate,
the provider still reaches the existing `PROVIDER_RECIPE_NOT_BOUND` result until
the `RealDataPlane`/T20 access-plan/T28 validation pipeline is injected into the
composition root. No fake `QualifiedGate`, receipt or in-memory production
fallback was added.

## Verification boundary

Rust compilation, tests, rustfmt, strict Clippy, native Windows execution and
live Qdrant qualification are **NOT_RUN** because the available execution
environment has no Rust toolchain or accepted live Qdrant artifact. No `PASS`,
T22 qualification, T30 product-spine acceptance or independent review is
claimed.
