# P00 contract challenge decisions

These decisions resolve implementation ambiguity without changing Architecture 8.4 Part I.

| ID | Ambiguity | Decision |
|---|---|---|
| PC-001 | H3.1 sketches `SourceOwnerGeneration(NonZeroU64)` while S7.2.1 defines a digest. | Part I wins: `SourceOwnerGeneration(Blake3Digest32)`; `OwnerEpoch` is the integer epoch. |
| PC-002 | Many schemas use several nullable fields to describe a variant. | Implement tagged enums and serialize only fields legal for the active tag. |
| PC-003 | S34 calls its list “key reason codes” while assignments introduced additional failures. | Separate public-provider, protocol, contract-validation and package-local namespaces. |
| PC-004 | Plans/results/handles contain grouped `object` placeholders. | Expand them into named subordinate records; no opaque object hides authority/currentness. |
| PC-005 | Timestamp, digest and byte encodings were not fixed. | Use `CANONICAL_TYPES.md`; private storage may differ but must round-trip exactly. |
| PC-006 | H4 lists ports but no package owns the traits. | `search-ports` owns shared traits; adapters implement them; daemon composes them. |
| PC-007 | Initial hello includes installation incarnation before negotiation. | Pairing/config supplies expected identity; hello must match; nil/wildcard is forbidden. |
| PC-008 | Package-local failures resemble public reasons. | They remain internal unless `REASON_CODES.md` registers a mapping. |
| PC-009 | A draft allowed stale/unreadable entries in emitted candidates. | S23 wins: emitted candidates are validated; failed nominations become non-evidence coverage gaps. |
| PC-010 | Derived schemas lowercased exact wire/state spelling. | Preserve explicit Architecture spelling for source ownership, cutover and publication states. |
| PC-011 | Recipe registry named outputs but output bodies remained opaque. | `RECIPE_RESULTS.md` defines every output and exact result union. |
| PC-012 | S7.3 uses `occurrence_sequence: u64`; draft narrowed it to nonzero. | Preserve `u64`. |
| PC-013 | Inspection/exploration/comparison could encode ambiguity together with resolved evidence. | Use nested tagged `resolved/ambiguous` variants; ambiguous variants contain no resolved evidence fields. |
| PC-014 | S26.1 requires opaque default handles while S26.2 lists the durable source fields bound by a handle. | Provider JSON carries an opaque `SearchSourceHandle`; S26.2 fields live in a server-owned durable record keyed by token digest. The record is never the bearer token. |
| PC-015 | A continuation token draft exposed binding, durability and plan fingerprint. | Public continuation is opaque lifecycle metadata only; binding/plan/fence/window/checkpoint fields remain in a server-owned tagged record. |

## Stop conditions

Stop with `CONTRACT_CHALLENGE` when Part I conflicts, a recipe needs implicit authority/scope, a public
reason lacks a mapping, a port needs vendor types, canonical bytes cannot be reproduced, mutually
exclusive states coexist, an invalid candidate would need to become evidence, or a public handle would
need to expose its server-side authority/currentness record.
