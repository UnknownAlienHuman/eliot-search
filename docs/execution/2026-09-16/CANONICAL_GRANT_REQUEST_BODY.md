# PR #185: canonical standalone-grant request body

Date: 2026-09-16. Tracking: product-test PR #185 and T20/W8 integration.

## Implemented protocol boundary

`search-provider-protocol::StandaloneGrantRequestV1` is now the bounded client
body for requesting a standalone read grant. It contains requested ceilings
only:

- observed binding and policy generations;
- memberships, corpus/reference-portfolio IDs and access partitions;
- modalities and recipe families;
- budget, sensitivity and disclosure ceilings;
- source-read/exact-scan permission ceilings;
- finite requested TTL.

The body cannot name a binding, installation, principal, operation identity,
grant identity, nonce, issue time or expiry time. Those remain server-side.

`encode_standalone_grant_request` emits one exact fixed-order UTF-8 JSON form.
UUID-like IDs use lower-case compact hexadecimal. Corpus and portfolio targets
use distinct `c:`/`p:` tags. Profile UTF-8 bytes use lower-case hexadecimal so
JSON escaping cannot create alternate encodings of the same profile. Sets are
emitted in canonical `BoundedSet` order.

`decode_standalone_grant_request` rejects whitespace, unknown/reordered fields,
upper-case hex, escaped strings, duplicate set items, non-versioned recipes,
leading-zero numbers, invalid permissions, zero generations/TTL, trailing bytes
and bodies above 1 MiB. It re-encodes the parsed value and requires exact byte
equality, leaving one accepted representation.

## Daemon mapping

`SessionBoundGrantAuthority::mint_protocol_request` maps the canonical body only
after receiving an active mutually authenticated standalone `BoundSession`.
The exact session supplies `binding_id`; the admitted envelope `RequestId` is
converted to a deterministic, domain-separated issuer operation identity. No
client field can substitute either value.

The mapped request then follows the existing double-read authority path:
server policy before issuance, non-widening mint, equal server policy after
issuance, otherwise no claims are returned.

## Regression inventory

Protocol tests cover canonical round trip, alternate hex representation,
duplicate set items, trailing bytes, invalid zero/empty/permission shapes and
oversize rejection before parsing.

Daemon tests cover successful mapping from authenticated session plus request
identity and rejection of a malformed protocol body before policy or issuer
access.

## Remaining wiring

This increment defines the exact body and server mapping but does not yet add a
new live line/pipe command. The next slice must bind these exact bytes into the
authenticated envelope body digest, register the grant command in the closed
command registry, retain the admitted request guard through authority execution
and return the canonical grant without routing the operation to the DIRECT
child. A concrete authoritative policy source is still required; token-file or
ACL state cannot fill that role.

## Required execution

```text
cargo +1.98.0 test --locked -p search-provider-protocol grant::tests
cargo +1.98.0 test --locked -p eliot-searchd --features wave4-query \
  grant_authority::tests
cargo +1.98.0 check --locked -p search-provider-protocol -p eliot-searchd \
  --all-targets --all-features
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-provider-protocol -p eliot-searchd \
  --all-targets --all-features -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo`, `rustc` and
`rustfmt` are absent). No T20/W8 acceptance, live grant round trip, runtime PASS
or independent review is claimed.
