# P00 contract challenge decisions

These decisions resolve implementation ambiguities without changing Architecture 8.4 Part I.

| ID | Ambiguity | Decision |
|---|---|---|
| PC-001 | H3.1 sketches `SourceOwnerGeneration(NonZeroU64)` while S7.2.1 defines a digest. | Part I wins: `SourceOwnerGeneration(Blake3Digest32)`. `OwnerEpoch` is the integer epoch. |
| PC-002 | Many schemas use several nullable fields to describe a variant. | Implement a tagged enum and serialize exactly the fields legal for its `kind`; impossible combinations fail validation. |
| PC-003 | S34 calls its list “key reason codes” while assignments introduced additional failures. | Use separate public-provider, protocol, contract-validation and package-local namespaces; only registered provider codes enter candidate/coverage results. |
| PC-004 | `SearchTaskPlan`, `SearchCandidateSet` and handle schemas contain grouped `object` placeholders. | Expand them into named subordinate records while preserving every load-bearing Part I axis; no opaque object may hide authority/currentness fields. |
| PC-005 | Timestamp, digest and byte encodings are not fixed for deterministic fixtures. | Use the canonical rules in `CANONICAL_TYPES.md`; storage adapters may use another private representation but must round-trip exactly. |
| PC-006 | H4 lists ports but no package owns the traits. | `search-ports` owns shared vendor-neutral traits; adapters implement them and the daemon composes them. |
| PC-007 | Initial provider `hello` includes installation incarnation before negotiation. | Pairing/configuration supplies the expected installation identity; hello must match it. Nil/wildcard incarnation is forbidden in the baseline. |
| PC-008 | Package-local typed failures resemble public reason codes. | They remain internal unless `REASON_CODES.md` registers an explicit stable mapping. Vendor strings never become provider reasons. |

## Stop conditions

A writer stops and raises `CONTRACT_CHALLENGE` when:

- Part I and this pack disagree on a load-bearing field;
- a recipe cannot be represented without adding authority or an implicit scope;
- a public reason has no registry entry or mapping;
- a port would need a vendor/native type;
- canonical bytes cannot be reproduced from the stated schema;
- a nullable/opaque field can encode two materially different states.
