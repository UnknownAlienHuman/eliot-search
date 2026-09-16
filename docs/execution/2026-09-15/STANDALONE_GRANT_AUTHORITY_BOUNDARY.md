# Standalone grant authority boundary

Date: 2026-09-15. Tracking: product-test PR #185 and W8 generic-edge integration.

## Implemented boundary

`bins/eliot-searchd/src/access_composition/grant.rs` implements the daemon-owned
`mint_standalone_grant` composition step without inventing an authentication or
signature protocol.

The client request is treated only as a set of requested ceilings. Before any
issuer call, composition requires the exact binding and policy generations and
verifies that requested memberships, corpus/portfolio identifiers, access
partitions, modalities, recipes and budget class are contained by the
authoritative capture. Sensitivity, disclosure, source-read, exact-scan and TTL
values may only narrow the authoritative policy. Empty executable scope fails
closed. A portfolio revision is retained only when the request actually includes
a portfolio; corpus-only grants cannot inherit an unrelated portfolio fence.

Grant identity, nonce and trusted timestamps remain behind
`StandaloneGrantIssuer`. Its receipt must echo the exact operation, binding and
generations, and its effective TTL may only narrow the request. Equal operation
plus equal template reconstructs the same material; equal operation plus changed
template is an explicit conflict. Unknown outcome stays typed and is never
replayed as a new mutation.

`BoundedStandaloneGrantIssuer` owns the finite process-incarnation idempotency
table. It retains exact operation/template/material triples, rejects identity
collisions with bounded retries and never evicts an operation identity to create
apparent capacity. Capacity exhaustion therefore fails closed. Restart drops
only this boot-local table; claims remain bound to the current boot ID and must
still pass normal live binding/revocation checks.

## Production runtime adapters

`bins/eliot-searchd/src/qualified_entropy.rs` is the single daemon owner for
direct operating-system CSPRNG access:

- Unix reads the exact requested byte count from `/dev/urandom`;
- Windows uses `BCryptGenRandom` with the system-preferred RNG;
- empty requests, unsupported platforms and incomplete reads fail closed;
- continuation tokens and standalone grants now call this same owner.

`QualifiedGrantEntropy` implements `GrantEntropySource` over that owner. It does
not derive identifiers from process IDs, clocks, token files or stored secrets.
The bounded issuer still rejects all-zero identities and collisions before a
grant can be returned.

`SystemGrantClock` implements the wall-clock side of `GrantTimeSource` without a
new date/time dependency. One `SystemTime` observation determines both canonical
RFC 3339 timestamps. Millisecond TTL arithmetic occurs before formatting; both
ends retain the same sub-microsecond truncation, so the requested integer TTL is
not widened by formatting. Pre-epoch time, arithmetic overflow, year 10000 or a
clock rollback behind the preceding process observation fails closed.

`production_standalone_grant_issuer` composes these two adapters with the finite
boot-local issuer. Constructing that issuer is still not authorization and does
not expose a wire operation.

The resulting `SearchReadGrantClaims` uses only canonical contract types and is
shape-validated before return. No local token, process identity, loopback
address, ACL, capability descriptor or handle becomes authority.

## Regression inventory

Focused tests cover:

- exact requested/authoritative intersection;
- portfolio revision inclusion only for requested portfolio scope;
- foreign membership rejection before issuer invocation;
- sensitivity/disclosure/permission non-widening;
- equal-operation reconstruction and conflicting-input rejection;
- stale policy generation rejection;
- foreign issuer receipt rejection;
- finite issuer capacity without operation-identity eviction;
- unique grant/nonce issuance and repeated-entropy collision rejection;
- canonical epoch/leap-day formatting and exact finite TTL windows;
- wall-clock rollback, zero TTL and out-of-contract year rejection;
- one daemon-native entropy owner shared by continuation and grant adapters.

Deterministic test entropy/time fakes remain local to tests. Their existence is
not product CSPRNG or native execution evidence.

## Remaining wiring

This increment provides the safe issuer interface, finite boot-local operation
owner, qualified OS entropy adapter and canonical system-clock adapter. It does
**not** authorize a client or make indexed query routing available. Provider
routing still requires:

1. a server-owned authenticated binding/policy snapshot source;
2. canonical grant-request framing;
3. exact connection/session binding of the returned claims;
4. access compilation and live rechecks before query, IDF, count and source work;
5. a declared recovery policy before any future durable cross-restart issuer;
6. executed native tests and independent exact-head review.

Until those are present, the provider query must remain unavailable rather than
accepting a local token, process identity or capability flag as a grant.

## Required execution

```text
cargo +1.98.0 test --locked -p eliot-searchd --features wave4-query \
  access_composition::grant::tests
cargo +1.98.0 test --locked -p eliot-searchd --features wave4-query \
  access_composition::system_grant::tests
cargo +1.98.0 test --locked -p xtask --test grant_runtime_adapter_ownership
cargo +1.98.0 check --locked -p eliot-searchd --all-targets --all-features
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p eliot-searchd -p xtask \
  --all-targets --all-features -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo` is absent).
No runtime PASS, W8/G4 acceptance or independent review is claimed.
