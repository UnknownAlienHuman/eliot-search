# Projection — complete generations before measured source-local deltas

Projection turns prepared units into bounded uploadable pages and matching evidence bindings. It owns no
redb/Qdrant effect, publication state or recovery ledger. Keep it inside preparation/publication unless a
substantial independent algorithm justifies the package.

## Current source seam

The baseline audit at `c8e38b6` found `plan_projection`/`PublicationAdmissionOwners` connected to fixtures rather
than a publisher; `PointSpec` discarded uploadable input for digests and cloned manifests. Recheck actual callers
at the code cut. Do not repair that proof protocol or preserve it through wrappers.

## Stage 2/3A assembler

Consume one complete captured inventory of preparation outcomes: exact source/prepared/ObjectRefs, explicit gaps,
unit text/ranges, selected structural context, physical profile and actual scope fields.

[Unitization](../../search-prep/search-unitizer/FUNCTIONS.md#representation-and-ownership) supplies one representation
with bounded descriptors, not another point model. Compose the selected retrieval text from those ranges into each
bounded page; no reparse or whole-document/per-unit string duplication. Derive backend IDs here, not in reusable
prepared artifacts. Bind the already-persisted prepared ObjectRef in evidence; never require a self-reference.

Stream bounded pages containing actual retrieval_text, required routing/filter payload and matching evidence.
A digest is not uploadable input. Validate complete selected outcomes and point/byte ceilings before effects;
an oversized source produces an explicit gap or build refusal, not silent partial-source completeness.

Derive deterministic collision-checked IDs within the never-reused physical generation. The key may include
generation, source/prepared identity, unit occurrence and role. Cross-generation continuity is unnecessary.
Sort/page only as required for deterministic build/readback; do not copy the whole corpus or create canonical
value trees, full-manifest caches or a baseline retain/close plan.

## Physical profile versus query scoring

The [lexical owner](../../search-lexical/FUNCTIONS.md) defines these separately:

- Physical profile: retrieval-text composition, tokenizer/index/resource settings and payload/filter schema.
  A physical-profile change creates a fresh generation.
- Query scoring: request-time BM25 k/b. Changing only k/b does not change projection input, point identity,
  pending build, physical collection or durable current view.

The bridge owns vendor schema conversion. Baseline pages contain no sparse BM25 inference vectors, fixed local
avg_len or locally generated identifier aliases.

## Stage 3B — only after failed Stage 3A budgets

Add a source-local delta only when the [delivery sequence](../../../docs/implementation/README.md#3b-measured-incremental-publication)
selects it. Compare bounded old/new descriptors within the same source/context, sort by point ID, and merge the
old source-prefix evidence into retain/create/fixed-close operations with complete-source evidence changes.

Reuse only unambiguous one-old/one-new occurrences with identical retrieval input/context. Equal text or ordinal
alone is not continuity. Ambiguous/unmatched units receive births fixed in the persisted pending delta;
disappear/reappear never reopens a closed interval. Preserve actual text/payload for replay.

Memory/cost is O(m log m + o) for m new/o old units plus bounded output, not constant. Validate the complete delta
before effects. An oversized delta uses a fresh generation or explicit gap. Custom per-source shards are not a
substitute for a measured publication strategy.

## Removal and verification

Delete whole ProjectionPlan/manifest copies, dual diff maps, digest-only output, revision-bound scoring IDs and
fixture-only admission/codec APIs with callers. Keep source ranges and collision checks.

After code run minimal locked check/Clippy for actual preparation/publication/bridge/evidence consumers. Then
check complete/empty/gap inventory paging, deterministic IDs, exact payload/evidence and bounded memory. Stage 3B
adds duplicate/shift/reorder/context/replay cases and measured write reduction only if selected.
