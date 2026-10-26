# Point identity — generation-bound baseline, stable occurrence identity only in Stage 3B

This package owns deterministic Qdrant address derivation and collision evidence only when that responsibility is
not simpler inside projection/publication. It owns no registry, source lifecycle, publication state or vendor I/O.

## Existing source

Current `s11` hashes a revision/coordinate-bearing eight-field key, projects a full BLAKE3 digest to a 128-bit
address and compares full identity evidence on readback. Useful mechanics are deterministic bounded encoding, a
complete identity distinct from the Qdrant locator and collision refusal. The current key shape/public names are
not compatibility contracts.

## Stage 2 / 3A baseline

Each physical Qdrant collection name is never reused. Point IDs only need deterministic uniqueness and collision
checking **within that generation**. A small key may bind:

```text
identity schema version
physical generation
source/prepared identity
unit occurrence within that complete prepared outcome
retrieval profile/role
```

Absolute citation range lives in the pinned evidence row. Cross-generation continuity, occurrence incarnation and
fixed closure are absent. A later generation may remint every point. This keeps #264 and restart-convergent Stage
3A independent of duplicate matching and stable identity design.

The address is a locator, not complete identity. Publication stores/readbacks enough evidence to reject an
occupied address with conflicting meaning. Do not build a mutable point registry or compatibility migration.

If the Stage 2/3A key is trivial, implement it privately in projection/publication and delete this package after
callers move. Package existence is not a reason to preserve S11.

## Stage 3B identity — conditional on failed budgets

Only when Stage 3A fails #215's predeclared budgets and #259 is activated, define a stable intra-generation key:

```text
key schema version
physical generation / incremental profile
stable source/projection grouping
occurrence incarnation
retrieval profile and role
exact retrieval-input identity
```

Unitizer/projection assigns `occurrence_incarnation`. Reuse only one-old/one-new unambiguous occurrence with the
same retrieval input/context. Equal text or duplicate ordinal is not continuity. Ambiguous/unmatched units receive
operation-fixed births persisted in the pending delta. Disappear/reappear gets a new birth and never reopens a
closed interval.

Full identity remains distinct from the 128-bit Qdrant address; collision blocks mutation. Exact revision,
representation and range remain in pinned evidence metadata.

## Cutover

For Stage 2/3A, delete revision-bound S11/legacy FNV registries and mint only the simple generation-bound profile in
a fresh collection.

If Stage 3B is selected later, change together:

- identity key/decoder/comparator;
- unitizer occurrence matching and projection assignment;
- Qdrant payload conversion/readback;
- publication delta/closure;
- evidence lookup.

Transition by building a fresh incremental-profile generation. Never dual-mint or reinterpret existing points.

## Checks

Baseline: deterministic generation-bound identity, distinct equal occurrences, collision refusal, exact evidence
lookup and old physical-view behavior.

Stage 3B if selected: unchanged unique occurrence reuse, ambiguous duplicates, changed input/profile/incarnation,
pending birth replay, disappear/reappear and measured work reduction versus Stage 3A.
