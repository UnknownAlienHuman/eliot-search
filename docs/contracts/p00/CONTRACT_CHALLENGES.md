# P00 contract challenge decisions

These decisions resolve implementation ambiguity without changing Architecture 8.4 Part I.

| ID | Ambiguity | Decision |
|---|---|---|
| PC-001 | H3.1 sketches integer owner generation while S7.2.1 defines a digest. | Part I wins: digest generation; separate integer owner epoch. |
| PC-002 | Nullable fields describe variants. | Use tagged enums with only legal fields. |
| PC-003 | S34 is a “key” reason list while assignments add failures. | Separate public, protocol, contract and local namespaces. |
| PC-004 | Plans/results/handles group load-bearing `object` values. | Expand named records; opaque objects cannot hide authority/currentness. |
| PC-005 | Canonical timestamp/digest/byte encodings absent. | Use `CANONICAL_TYPES.md`. |
| PC-006 | H4 ports have no owner. | `search-ports` owns traits; adapters implement; daemon composes. |
| PC-007 | Hello carries incarnation before negotiation. | Pairing/config supplies expected identity; wildcard is forbidden. |
| PC-008 | Local failures resemble public reasons. | Internal unless explicit mapping exists. |
| PC-009 | Draft emitted stale/unreadable candidates. | S23 wins: validated candidates only; failures are non-evidence gaps. |
| PC-010 | Draft lowercased exact state spelling. | Preserve Architecture wire/state spelling. |
| PC-011 | Recipe outputs remained opaque. | Define all output bodies and exact result union. |
| PC-012 | Draft narrowed occurrence sequence. | Preserve architectural `u64`. |
| PC-013 | Ambiguity could coexist with evidence. | Closed `resolved/ambiguous` variants. |
| PC-014 | S26.1 opaque handles vs S26.2 durable bound fields. | Wire handle is opaque; detailed durable fields are server record keyed by token digest. |
| PC-015 | Continuation token exposed binding/plan. | Public token opaque; server record owns binding/plan/fence/window/checkpoint. |
| PC-016 | Task plan hid S14 snapshot revisions in generic dependency digests. | Add exact `QuerySnapshotFence`; generic dependencies cannot replace any S14 field. |
| PC-017 | Snapshot isolation and live restrictive revalidation could be conflated. | `ResultFence` preserves planned snapshot and separately records latest emission owner/security fences. |
| PC-018 | P00 schemas use `RecipeIdV1`, `RecipeBodyV1`, `ComparisonAxis`, `ProtocolRange` and `PackageOpaque` without explicit registry definitions. | `TYPE_COMPLETIONS.md` defines their closed shapes, owners, canonical/compatibility rules and no-vendor/no-serialization boundaries. Local aliases are forbidden. |

## Stop conditions

Stop when Part I conflicts; recipe needs implicit authority; reason mapping is absent; port needs vendor
types; canonical bytes cannot reproduce; mutually exclusive states coexist; invalid evidence would be
required; public handle would expose server record; a named helper type lacks a closed definition; or a
generic digest would hide a load-bearing snapshot/security field.
