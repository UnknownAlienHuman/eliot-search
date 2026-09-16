# PR #185: authenticated standalone-grant command kernel

Date: 2026-09-16. Tracking: product-test PR #185 and T20/W8 integration.

## Implemented protocol envelope

Standalone grants no longer need to masquerade as a health/version/shutdown
shell command. `AuthenticatedStandaloneGrantEnvelope` has its own proof domain:

```text
ELIOT-STANDALONE-GRANT-REQ-v1
```

The exact transcript binds protocol version, per-incarnation server nonce,
opaque request ID and the BLAKE3 digest of the exact canonical grant-request
body. Binding, installation, principal, policy and issued-grant identity remain
absent.

The envelope has one strict fixed-order JSON representation and uses the
existing bounded length-prefixed frame. Decoder constraints include:

- independent 512-byte JSON ceiling;
- lower-case fixed-size hex only;
- no whitespace, escapes, field reordering, unknown fields or trailing bytes;
- non-zero server nonce;
- exact supported protocol version;
- exact re-encoding equality.

The W1 `ControlCommand` registry remains unchanged. The grant envelope cannot be
forwarded accidentally to the DIRECT child as a shell command.

## Implemented daemon command kernel

`execute_standalone_grant_command` composes the completed pieces in fixed order:

1. hash exact request-body bytes;
2. authenticate the grant-specific envelope and exact digest through the
   canonical `BoundSession`;
3. retain the connection-owned request guard;
4. decode the one canonical grant body;
5. invoke `SessionBoundGrantAuthority` for exact binding/policy intersection;
6. record one terminal and release one in-flight slot.

Admission failure returns no terminal status because sequence/replay/in-flight
state was untouched. Malformed authenticated bodies complete as `failed`.
Authority failures that may occur after issuer effects — second policy read
failure/mismatch, changed policy, issuer outcome unknown, mismatched receipt or
invalid issued material — complete as `outcome_unknown`. This prevents a
possible issued grant from being relabelled ordinary failure. Successful claims
are returned only after recording the `ok` terminal.

## Regression inventory

Protocol tests cover canonical JSON/frame round trip, proof transcript/body
binding, alternate representation rejection, unsupported versions, zero nonce
and oversize rejection.

Daemon tests cover:

- successful exact body → policy intersection → grant → terminal release;
- body mismatch before request identity/sequence consumption;
- successful reuse of the same identity/sequence after corrected bytes;
- malformed authenticated body producing one failed terminal;
- post-issuer policy change producing one outcome-unknown terminal;
- duplicate terminal rejection after command completion.

## Remaining live integration

The command kernel is transport-neutral. The existing development loopback
proxy still has only token-file pairing and a parallel `ProviderRouter`; it does
not possess the authoritative durable `BindingContext` required by this path.
The next live integration must first supply the real installation incarnation,
durable binding record, mutually verified pairing and concrete
`StandaloneGrantPolicySource`. Only then may the transport decode the dedicated
envelope/body frames and invoke this kernel.

No token-file, ACL, loopback address or client body field has been promoted to
authority.

## Required execution

```text
cargo +1.98.0 test --locked -p search-provider-protocol grant::envelope::tests
cargo +1.98.0 test --locked -p search-provider-protocol binding::tests
cargo +1.98.0 test --locked -p eliot-searchd --features wave4-query \
  grant_command::tests
cargo +1.98.0 check --locked -p search-provider-protocol -p eliot-searchd \
  --all-targets --all-features
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-provider-protocol -p eliot-searchd \
  --all-targets --all-features -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo`, `rustc` and
`rustfmt` are absent). No live grant round trip, T20/W8 acceptance, runtime PASS
or independent review is claimed.
