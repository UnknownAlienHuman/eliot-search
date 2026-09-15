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

`BoundedStandaloneGrantIssuer` now owns the finite process-incarnation
idempotency table. It delegates every random byte to `GrantEntropySource` and
every canonical issue/expiry window to `GrantTimeSource`. It retains exact
operation/template/material triples, rejects identity collisions with bounded
retries and never evicts an operation identity to create apparent capacity.
Capacity exhaustion therefore fails closed. Restart drops only this boot-local
table; the template and resulting claims remain bound to the current boot ID and
must still pass normal live binding/revocation checks.

The resulting `SearchReadGrantClaims` uses only canonical contract types and is
shape-validated before return. No local token, process identity, loopback
address, ACL, capability descriptor or handle becomes authority.

## Regression inventory

The focused module tests cover:

- exact requested/authoritative intersection;
- portfolio revision inclusion only for requested portfolio scope;
- foreign membership rejection before issuer invocation;
- sensitivity/disclosure/permission non-widening;
- equal-operation reconstruction and conflicting-input rejection;
- stale policy generation rejection;
- foreign issuer receipt rejection;
- finite issuer capacity without operation-identity eviction;
- unique grant/nonce issuance and repeated-entropy collision rejection.

The entropy and time implementations used by tests are deterministic fakes. They
are not product CSPRNG or clock evidence.

## Remaining wiring

This increment provides the safe issuer interface, finite boot-local operation
owner and pure composition step. It does **not** expose a new wire operation or
pretend that a test entropy/clock source is production authority. Provider
routing still requires:

1. a server-owned binding/policy snapshot source;
2. a qualified operating-system CSPRNG adapter and canonical clock adapter;
3. canonical grant-request framing;
4. connection binding of the returned claims;
5. access compilation and live rechecks before query/IDF/source work;
6. a declared recovery policy for any future durable cross-restart issuer.

Until those are present, the current provider query remains unavailable rather
than accepting a local token as a grant.

## Required execution

```text
cargo +1.98.0 test --locked -p eliot-searchd --features wave4-query \
  access_composition::grant::tests
cargo +1.98.0 check --locked -p eliot-searchd --all-targets --all-features
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p eliot-searchd --all-targets --all-features -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo` is absent).
No runtime PASS, W8/G4 acceptance or independent review is claimed.
