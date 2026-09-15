# Standalone grant authority boundary

Date: 2026-09-15. Tracking: product-test PR #185 and W8 generic-edge integration.

## Implemented boundary

`bins/eliot-searchd/src/access_composition/grant.rs` now implements the daemon-owned
`mint_standalone_grant` composition step without inventing an authentication or
signature protocol.

The client request is treated only as a set of requested ceilings. Before any
issuer call, composition requires the exact binding and policy generations and
verifies that requested memberships, corpus/portfolio identifiers, access
partitions, modalities, recipes and budget class are contained by the
authoritative capture. Sensitivity, disclosure, source-read, exact-scan and TTL
values may only narrow the authoritative policy. Empty executable scope fails
closed.

Grant identity, nonce, trusted timestamps and durable idempotency remain behind
the injected `StandaloneGrantIssuer`. Its receipt must echo the exact operation,
binding and generations, and its effective TTL may only narrow the request.
Equal operation plus equal template reconstructs the same material; equal
operation plus changed template is an explicit conflict. Unknown outcome stays
typed and is never replayed as a new mutation.

The resulting `SearchReadGrantClaims` uses only canonical contract types and is
shape-validated before return. No local token, process identity, loopback
address, ACL, capability descriptor or handle becomes authority.

## Regression inventory

The focused module tests cover:

- exact requested/authoritative intersection;
- foreign membership rejection before issuer invocation;
- sensitivity/disclosure/permission non-widening;
- equal-operation reconstruction and conflicting-input rejection;
- stale policy generation rejection;
- foreign issuer receipt rejection.

## Remaining wiring

This increment provides the missing safe issuer interface and pure composition
step. It does **not** fabricate a concrete grant issuer or expose a new wire
operation. Provider routing still requires:

1. a server-owned binding/policy snapshot source;
2. a trusted issuer implementation with CSPRNG, clock and durable operation
   recovery;
3. canonical grant-request framing;
4. connection binding of the returned claims;
5. access compilation and live rechecks before query/IDF/source work.

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
