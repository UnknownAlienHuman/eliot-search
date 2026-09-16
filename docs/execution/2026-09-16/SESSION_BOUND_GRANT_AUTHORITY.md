# PR #185: session-bound standalone grant authority

Date: 2026-09-16. Tracking: product-test PR #185 and T20/W8 integration.

## Implemented composition

`SessionBoundGrantAuthority` now composes the existing non-widening grant
minter only from an active `search-provider-protocol::BoundSession`, an injected
server-owned policy source and an injected issuer.

The fixed order is:

1. require an active mutually authenticated provider session;
2. require the authenticated peer role to be `StandaloneCli`;
3. read the current active binding-policy snapshot from the server-owned source;
4. bind that snapshot to the session's exact binding ID and installation
   incarnation;
5. run the existing scope/ceiling/generation/TTL intersection and issuer;
6. read policy again and require exact equality before returning claims.

The source contract must return an error for missing, revoked, expired,
unreadable or outcome-unknown binding state. Client request fields cannot create
or repair policy. Source errors are deliberately collapsed to a binding-safe
`DAEMON_GRANT_POLICY_UNAVAILABLE` result.

If policy changes or becomes unavailable after issuer mutation, no grant claims
are returned. The issuer retains the operation identity, so callers cannot
blindly retry the same operation under changed authority. Later policy changes
remain fenced by binding/policy/revocation generations and normal live access
rechecks.

## Regression inventory

Focused tests cover:

- successful issuance only after two equal policy reads;
- inactive and non-standalone sessions denied before policy access;
- foreign binding and foreign installation-incarnation policies denied before
  issuer invocation;
- policy change and second-read failure after issuance discarding claims;
- preservation of the exact underlying non-widening mint reason.

The tests build a real `BoundSession` through hello, mutual pairing,
`authenticate_binding` and `BoundSession::open`; they do not construct a fake
binding token.

## Remaining wiring

This increment still does not invent a binding database or access policy. The
next integration slice must implement `StandaloneGrantPolicySource` over the
single authoritative control/live snapshot owner, expose a canonical bounded
grant request inside the authenticated protocol, retain the authority for the
connection lifetime and feed returned claims into the existing pre-retrieval
access compiler. Indexed routing remains unavailable until that chain and the
real query executor are both present.

## Required execution

```text
cargo +1.98.0 test --locked -p eliot-searchd --features wave4-query \
  grant_authority::tests
cargo +1.98.0 check --locked -p eliot-searchd --all-targets --all-features
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p eliot-searchd --all-targets --all-features \
  -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo`, `rustc` and
`rustfmt` are absent). No T20/W8 acceptance, runtime PASS, indexed-query
availability or independent review is claimed.
