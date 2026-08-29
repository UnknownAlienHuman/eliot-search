# W8 hardening contract — `search-provider-protocol`

This packet extends the W1/W4 `FUNCTIONS.md` for P14/G4. It owns protocol/binding mechanics only; grant
semantics remain in `search-access`, handle state in `search-handles`/`search-continuation`, and
capability source data in daemon composition.

## `issue_pairing_challenge`

```text
issue_pairing_challenge(peer_role, transport_peer, policy, clock, randomness)
  -> Result<PairingChallenge, ProtocolError>
```

Creates one CSPRNG challenge bound to installation incarnation, peer role, transport peer identity,
protocol range, pairing generation and finite expiry. The plaintext challenge is never logged or
persisted beyond the bounded pairing state required by the accepted design.

Minting is intentionally non-idempotent. A lost challenge is abandoned/expired; it is not regenerated
from stable identities.

## `verify_pairing_proof`

```text
verify_pairing_proof(challenge, proof, peer_identity, clock)
  -> Result<VerifiedPairing, ProtocolError>
```

Rejects replay, expiry, role substitution, transport-peer mismatch, installation/incarnation mismatch,
wrong protocol range and already-consumed challenge. Verification consumes the challenge exactly once.

Success proves peer binding input only; it grants no source membership or recipe authority.

## `commit_binding`

```text
commit_binding(verified_pairing, requested_profile_set, policy, mutation, binding_store)
  -> Result<BindingReceipt, ProtocolError>
```

The daemon supplies authoritative permitted profiles/disclosure ceiling. Protocol persists only the
bounded binding record through an injected port.

Same mutation identity plus equal canonical input reconstructs the existing receipt. Same identity plus
different peer/profile/ceiling fails. A timeout after possible commit is recovered by operation identity
and binding digest before retry.

Acknowledgement occurs only after the durable record and live binding/revocation snapshot are visible.

## `revoke_binding`

```text
revoke_binding(binding_id, expected_generation, reason, mutation, ports)
  -> Result<BindingRevocationReceipt, ProtocolError>
```

Orders:

```text
durable binding revocation
→ live binding snapshot publication
→ active connection drain
→ request cancellation
→ dependent handle/continuation invalidation request
→ acknowledgement
```

Cancellation after durable commit cannot report rollback. Recovery completes publication/invalidation
or leaves the binding domain fail-closed.

## `open_authenticated_connection`

```text
open_authenticated_connection(hello, transport_peer, binding_snapshot, limits)
  -> Result<ConnectionGuard, ProtocolError>
```

Requires accepted major/minor negotiation, active exact binding, pairing generation, installation
incarnation and peer identity. Initializes sequence/in-flight/event state at one fresh connection ID.

A named-pipe ACL or local-user match alone is insufficient. A stale/revoked binding fails without
revealing hidden membership/capability data.

## `project_capability_descriptor`

```text
project_capability_descriptor(authoritative, binding, disclosure, limits)
  -> Result<BindingFilteredCapabilityDescriptor, ProtocolError>
```

Filters before count/readiness/reason aggregation. Only visible opaque memberships and permitted
profiles/recipes survive. Inaccessible names, paths, corpus identifiers, counts and reason detail cannot
influence output.

Descriptor digest binds protocol, installation/incarnation, owner epoch, access generation, inventory,
visible epoch, route revision and filtered membership readiness. It is planning data, never a permit.

Equal captured authoritative/binding inputs yield byte-identical descriptor bytes/digest.

## `admit_generic_request`

```text
admit_generic_request(connection, envelope, capability_digest, live_binding, limits)
  -> Result<ProviderRequestGuard, ProtocolError>
```

In addition to the base function packet, validates:

- binding still active and peer identity unchanged;
- exact request recipe is in the P00 eleven-recipe registry and descriptor set;
- no adapter-specific core recipe/tag;
- grant/request fields are structurally bounded before access validation;
- capability digest staleness cannot widen behavior;
- sequence/request ID/in-flight/deadline limits.

The guard carries cancellation capability and binding identity but no reusable grant decision.

## `emit_binding_filtered_descriptor`

```text
emit_binding_filtered_descriptor(connection, descriptor)
  -> Result<DescriptorEventReceipt, ProtocolError>
```

Emits only after authentication and only for the connection binding. Descriptor event sequence is
monotonic and bounded. A profile activation/deactivation must publish one coherent descriptor snapshot;
mixed handler/descriptor availability is rejected by daemon integration.

## `route_expand_handle`

```text
route_expand_handle(request_guard, expand_handle_request, live_binding, handle_port, context)
  -> Result<RecipeResultV1, ProtocolError>
```

Performs protocol/binding checks, then delegates all token lookup, grant/live security, source readback,
range and TTL semantics to the handle/continuation owner. Protocol never parses token contents or caches
an expansion permit. The returned result is emitted only after a final live binding check.

## `close_connection`

```text
close_connection(connection, reason, cancellation_port)
  -> DisconnectReceipt
```

Idempotently transitions to draining/closed, stops new envelopes, cancels every in-flight request and
releases connection-owned request guards. It does not delete durable handles or fabricate completion of
mutations that may have committed.

## Required W8 failures

- `PAIRING_CHALLENGE_EXPIRED`
- `PAIRING_CHALLENGE_REPLAYED`
- `PAIRING_PROOF_INVALID`
- `PAIRING_ROLE_MISMATCH`
- `BINDING_NOT_ACTIVE`
- `BINDING_REVOKED`
- `BINDING_OPERATION_CONFLICT`
- `CAPABILITY_DESCRIPTOR_STALE`
- `CAPABILITY_DISCLOSURE_VIOLATION`
- `CLIENT_ADAPTER_AUTHORITY_VIOLATION`
- `REQUEST_RECIPE_NOT_AVAILABLE`
- `HANDLE_EXPANSION_REAUTHORIZATION_FAILED`

## Required W8 fixtures

- single-use challenge replay/expiry/role/incarnation/peer matrix;
- commit timeout before/after durable binding write;
- revocation crash at every ordered phase;
- authenticated descriptor only;
- hidden membership/name/count/reason noninterference;
- stale descriptor cannot widen request;
- exact eleven-recipe closure and adapter-specific recipe rejection;
- handle expansion delegates and rechecks binding before emission;
- disconnect cancels/release request state without mutating durable evidence;
- public API and logs contain no pairing proof, token, grant decision or store/vendor type.
