# P00 contract challenge decisions

These decisions resolve implementation ambiguity without changing Architecture 8.4 Part I.

| ID | Ambiguity | Decision |
|---|---|---|
| PC-001 | H3.1 sketches `SourceOwnerGeneration(NonZeroU64)` while S7.2.1 defines a digest. | Part I wins: `SourceOwnerGeneration(Blake3Digest32)`. `OwnerEpoch` is the integer epoch. |
| PC-002 | Many schemas use several nullable fields to describe a variant. | Implement tagged enums and serialize only fields legal for the active `kind`; impossible combinations fail. |
| PC-003 | S34 calls its list “key reason codes” while assignments introduced additional failures. | Separate public-provider, protocol, contract-validation and package-local namespaces. |
| PC-004 | Plans/results/handles contain grouped `object` placeholders. | Expand them into named subordinate records; no opaque object hides authority/currentness fields. |
| PC-005 | Timestamp, digest and byte encodings were not fixed for deterministic fixtures. | Use `CANONICAL_TYPES.md`; private storage may differ but must round-trip exactly. |
| PC-006 | H4 lists ports but no package owns the traits. | `search-ports` owns shared vendor-neutral traits; adapters implement them; daemon composes them. |
| PC-007 | Initial hello includes installation incarnation before negotiation. | Pairing/config supplies expected identity; hello must match; nil/wildcard incarnation is forbidden. |
| PC-008 | Package-local typed failures resemble public reason codes. | They remain internal unless `REASON_CODES.md` registers an explicit mapping. |
| PC-009 | A draft candidate schema allowed stale/unreadable entries in `SearchCandidateSet.candidates`. | S23 wins: emitted candidates are validated; stale/unreadable/inaccessible nominations become coverage validation gaps with no evidence excerpt. |
| PC-010 | Derived schemas normalized exact state-machine/wire enum spelling to lowercase. | Preserve explicit Architecture wire/state spelling for source ownership, cutover and publication states. |
| PC-011 | Recipe registry named outputs but several output bodies remained opaque. | `RECIPE_RESULTS.md` defines every v1 output and the exact tagged result union. |
| PC-012 | S7.3 declares `occurrence_sequence: u64`; a draft projection narrowed it to nonzero. | Preserve `u64`; the owning implementation may define initial sequencing without changing the shared field type. |

## Stop conditions

A writer stops and raises `CONTRACT_CHALLENGE` when:

- Part I and this pack disagree on a load-bearing field;
- a recipe cannot be represented without adding authority or implicit scope;
- a public reason has no registry entry/mapping;
- a port would need a vendor/native type;
- canonical bytes cannot be reproduced;
- a nullable/opaque field encodes materially different states;
- an invalid candidate would need to appear as evidence to represent a gap.
