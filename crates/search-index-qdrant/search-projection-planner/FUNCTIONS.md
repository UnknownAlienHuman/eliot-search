# Projection — complete generation first, changed-source delta only when required

At `c8e38b6`, `plan_projection` and `PublicationAdmissionOwners` feed package/process fixtures rather than a
connected publisher. `PointSpec` discards uploadable input in favour of digests, clones manifests and creates
conflicting size ceilings. Do not repair that whole-plan platform.

## Stage 2 / 3A complete-generation assembler

Preparation supplies a complete bounded selected inventory of outcomes for one generation:

- exact source/prepared/ObjectRefs and explicit gaps;
- unit retrieval text and compact evidence coordinates;
- selected retrieval profile and scope fields.

Stream or page the complete inventory into bounded immutable `PendingGenerationBuild` pages. Each point contains
actual Qdrant 1.20 retrieval-text payload and filter/evidence keys; a digest is not uploadable input.

Stage 2/3A point IDs need only be deterministic and collision-checked **within the never-reused physical
generation**. They may bind generation + source/prepared/unit occurrence/role and be reminted in the next
generation. Cross-generation stable occurrence identity is not required.

Sort/page only as required by deterministic build/readback and bounded memory. Do not create a globally copied
UnitSet, canonical-value tree, manifest cache or a changed-source retain/close plan for the baseline.

Complete pages are validated before backend effects. A source/outcome ceiling produces an explicit gap or refuses
the build; never publish half a selected source while calling the generation complete.

## Qdrant 1.20 retrieval profile

Output one explicit retrieval-text payload field plus compact scope/evidence metadata for the scoring full-text
index. The bridge owns vendor conversion and schema. Baseline output does not contain sparse BM25 document/query
vectors or fixed `avg_len`.

Text processing—`code` tokenizer, lowercase/stemmer/stopword choices—and query-time `k`/`b` are part of the
retrieval profile. A changed profile creates a new physical generation.

## Stage 3B changed-source delta — conditional

Only for corpus classes where Stage 3A fails #215 budgets, add the source-local diff:

1. preparation supplies one bounded changed source product;
2. unitizer performs conservative occurrence matching;
3. point descriptors are sorted by point ID;
4. read the old source-prefix evidence range;
5. merge into retain/create/fixed-close operations and complete-source evidence changes;
6. persist actual retrieval text/payload and operation-fixed births in one fixed pending delta.

Reuse only one-old/one-new unambiguous occurrences with unchanged retrieval input/context. Equal text or duplicate
ordinal is not continuity. Ambiguous/unmatched units receive fixed new births. A changed input receives a new
point; coordinate-only movement may update evidence.

Memory/cost is O(m log m + o) for m new/o old units plus bounded output, not constant. Validate the complete-source
delta before Qdrant effects. Oversize delta falls back to full fresh generation or an explicit gap, never partial
source publication.

Custom per-root/source Qdrant shards are not a shortcut for this delta because full-text/BM25 statistics are
shard-local and cross-shard portfolio scoring would need another fusion owner.

## Removal

Remove whole `ProjectionPlan`/manifest copies, two full diff maps, digest-only output, revision-bound S11/scoring
IDs, artificial allocation arithmetic and test-only admission proof APIs. Stage 2/3A assembly may live directly in
publication/preparation. Keep a separate planner package only if Stage 3B is selected and the source-local merge is
substantial enough to justify it.

## Checks and measurements

Stage 2/3A: complete inventory paging, deterministic generation-bound IDs, exact payload/readback, zero-unit/gap
outcomes, bounded memory and build measurement.

Stage 3B if selected: edit at file start, repeated blocks, insert/remove/reorder duplicate, changed heading/context,
disappear/reappear, oversize source, pending replay and actual retained/new/closed points/Qdrant writes. Cache hits
alone do not prove useful incrementality.
