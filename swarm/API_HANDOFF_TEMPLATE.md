# Public API and port handoff

## Identity

- Package:
- Ticket ID / lease ID:
- Base commit:
- Final commit:
- Assignment SHA-256:
- API/schema version:
- Canonical API/schema digest:

## Public surface

### Shared contract types used

List exact `search-contracts` types and versions. Confirm no local look-alike shared record was added.

### Package-owned opaque types

List opaque handles/state descriptors and their ownership/lifetime semantics.

### Ports implemented

For every operation record:

| Port / operation | Inputs | Outputs | Preconditions | Postconditions | Idempotency | Cancellation/deadline | Typed failures |
|---|---|---|---|---|---|---|---|

### Ports consumed

| Producer package | Accepted API digest | Operations consumed | State referenced only opaquely |
|---|---|---|---|

## Concurrency and resource bounds

Document serialization/actor rules, queue limits, byte/item limits, ownership of cancellation and any
RAII guards. `unbounded` requires an architecture decision, not an omission.

## Serialization and compatibility

- Canonical serialization/profile:
- Unknown-field behavior:
- Backward/forward compatibility:
- Breaking-change procedure:
- Generated schema/fixture refs:

## Vendor/native isolation

Confirm public API contains none of:

- vendor request/response types;
- raw database/process/file handles;
- raw collection/table/key names;
- credentials or secret material;
- client authority or reusable authorization decisions.

## Conformance fixture

Provide a fake/in-memory conformance fixture that consumers can use without importing implementation
internals or running the vendor adapter.

## Evidence

- Contract/property tests:
- Compile-fail/public-surface guards:
- Raw command/output refs:
- Known unavailable checks:

## Reviewer receipt

- Digest reproduced:
- Semantics complete:
- Vendor isolation verified:
- Compatibility classification:
- Accepted/rejected with reasons:
