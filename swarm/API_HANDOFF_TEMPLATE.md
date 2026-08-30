# Public API and port manifest guide

This document describes the immutable public-surface artifact referenced by a package submission and
accepted package handoff. It is not itself an assignment ticket, lease, package handoff or authority
record.

## Identity

- package and stage;
- exact base and final commits;
- API/schema manifest version;
- canonical API/schema digest;
- optional configuration digest using explicit `OptionalV1`;
- fixture/golden digest set;
- public error/reason digest;
- compatibility class.

The manifest artifact is referenced through `ImmutableArtifactRef`; its SHA-256 is recomputed from exact
artifact bytes.

## Shared contract types used

List exact `search-contracts` records and versions. Confirm no local look-alike shared record was added.

## Package-owned opaque types

List opaque handles/state descriptors and their ownership, lifetime and revocation semantics.

## Ports implemented

For every public operation:

| Port / operation | Inputs | Outputs | Preconditions | Postconditions | Idempotency | Cancellation/deadline | Typed failures |
|---|---|---|---|---|---|---|---|

## Ports consumed

| Producer package | Accepted handoff ref | Accepted API digest | Operations consumed | State remains opaque |
|---|---|---|---|---|

A mutable branch or dependency source tree is never a consumed handoff.

## Concurrency and resource bounds

Document serialization/actor rules, queue limits, byte/item limits, cancellation ownership and RAII
guards. `unbounded` requires an architecture decision, not an omission.

## Serialization and compatibility

- canonical serialization/profile;
- unknown-field behavior;
- backward/forward compatibility;
- breaking-change and supersession procedure;
- generated schema/fixture refs;
- closed `ConsumerActionCode` entries required from downstream packages.

## Vendor/native isolation

Confirm the public surface contains none of:

- vendor request/response types;
- raw database/process/file handles;
- raw collection/table/key names;
- credentials or secret material;
- client authority or reusable authorization decisions.

## Conformance fixture

Provide a fake/in-memory conformance fixture that consumers can use without importing implementation
internals or running a vendor adapter.

## Evidence

- contract/property tests;
- compile-fail/public-surface guards;
- raw command/output refs;
- explicit unavailable checks;
- reviewer reproduction result.

## Acceptance boundary

The candidate manifest becomes accepted only when:

1. it is bound into an immutable package submission;
2. an independent review reproduces its digest and semantics;
3. the integration owner publishes a `package_handoff_v1` under
   `swarm/handoffs/<package>/<handoff_id>.toml`.

The API digest is public-surface identity, not accepted-record path identity.
