# PR #185: body-bound session lifecycle

Date: 2026-09-16. Tracking: product-test PR #185 and T20/W8 integration.

## Corrected lifecycle gap

The canonical `BoundSession` previously authenticated an envelope and retained
an in-flight guard, but it did not compare the envelope's authenticated body
digest with the digest of exact external request bytes. It also exposed cancel
and disconnect without a session-owned terminal completion method. An adapter
could therefore duplicate lifecycle ownership or release a slot through a path
that did not record the single terminal state in the canonical guard.

`BoundSession::admit_body_bound` now fixes the admission order:

1. active mutually paired session;
2. exact negotiated version and server nonce;
3. keyed envelope proof;
4. constant-work equality of authenticated and adapter-observed body digests;
5. deadline and finite in-flight capacity;
6. sequence/replay mutation;
7. retained connection-owned request guard.

A body mismatch returns `PROTOCOL_INVALID_BODY` before sequence, replay or
in-flight state changes. The same sequence and request identity remain usable
with the correct exact body.

`BoundSession::complete_request` records one terminal class and releases one
in-flight slot. The terminal guard remains retained until disconnect so a
second terminal is rejected as `PROTOCOL_DUPLICATE_TERMINAL`. A cancelled
request can complete only as cancelled and cannot be relabelled success. A
missing non-cancelled in-flight slot is contradictory state and quarantines the
session.

The implementation is split into `binding/lifecycle.rs`; `binding.rs` remains
the binding/session facade and its tests move to `binding/tests.rs`.

## Regression inventory

Tests cover:

- body mismatch before any mutable admission state;
- successful reuse of the same first sequence after a rejected body;
- retained in-flight ownership through execution;
- exact slot release on terminal completion;
- duplicate terminal rejection;
- cancellation followed by success rejection;
- cancellation followed by one cancelled terminal acknowledgement.

Existing hello, role-substitution, incarnation and pairing-token tests are
preserved after the module split.

## Remaining live integration

The development loopback proxy still uses a separate `ProviderRouter` built
from a token-file pairing shim. It does not possess the durable authoritative
`BindingContext` and policy source required by `SessionBoundGrantAuthority`.
This increment deliberately does not fabricate those identities or treat the
loopback address/token/ACL as authority.

The next live slice must replace the parallel router with the canonical
`BoundSession` only after the runtime can supply the exact durable binding,
installation incarnation and mutually verified pairing token. It must then
bind the canonical standalone-grant body digest before calling the existing
session-bound grant authority and preserve the guard through terminal response
sealing.

## Required execution

```text
cargo +1.98.0 test --locked -p search-provider-protocol binding::tests
cargo +1.98.0 test --locked -p search-provider-protocol --test pairing_envelopes
cargo +1.98.0 check --locked -p search-provider-protocol --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-provider-protocol --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo`, `rustc` and
`rustfmt` are absent). No live grant round trip, T20/W8 acceptance, runtime PASS
or independent review is claimed.
