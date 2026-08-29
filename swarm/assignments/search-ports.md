# `search-ports` implementation packet

**Path:** `crates/search-ports`  
**Capability:** C00 vendor-neutral operations  
**Delivery:** W0 / P00  
**Gate:** CONDITIONAL on accepted `search-contracts` API/schema digest

Read `CANONICAL_TYPES.md`, `TYPE_REGISTRY.md` port-support section, `SUPPORT_SCHEMAS.md` port note,
`PORT_OPERATIONS.md`, `REASON_CODES.md` and accepted contracts handoff.

## Mission

Publish the shared vendor-neutral trait surface and port-support types without selecting an executor,
database, OS API or vendor client.

## Owns

`OperationContext`, `MutationIdentity`, `PortReceipt`, bounded page/stream support, shared port traits,
idempotency/cancellation/deadline/bounds semantics and conformance fake interfaces.

## Does not own

Provider wire/domain/server records, adapter implementations, runtime state, domain rules, executor
choice, generic string errors or unbounded streams.

## Port groups

Runtime/control; secrets/process; source admission/inventory/ownership/readback/residency; preparation
and model providers; index data/admin and pins; access/overlay/exact/handles.

## Invariants

No vendor/native types; read operations require no durable idempotency row; mutations require explicit
operation identity; every blocking operation supports deadline/cancellation; streams remain bounded;
capability handles grant only documented operations; errors are typed/mapped.

## Evidence

Public vendor-type guard; complete method semantics; fake timeout/cancellation/partial/stale-generation
conformance; read/mutation idempotency tests; public API/method digest; dependency guard allows only
`search-contracts`.

Target `src/` ≤5,500 lines; split review before 8,500 total; hard stop 10,000 including conformance tests.
