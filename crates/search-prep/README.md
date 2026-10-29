# Preparation — one derived product, one structural UnitSet

A retained revision is read once, materialized once, divided into useful source units and converted into the
retrieval/evidence inputs selected by the published profile. Preparation owns derived data, not source truth,
current control state, Qdrant mutation or query lifetime.

Read the [architecture](../../docs/architecture/README.md) and
[implementation guide](../../docs/implementation/README.md).

## Connected baseline

```text
immutable retained ObjectRef
→ one materializer read/decode/parse
→ MaterializationProduct
→ structure-first UnitSet and ViewManifest entries
→ retrieval text / optional selected vectors
→ one prepared artifact ObjectRef
→ current prepared reference in redb
→ publication
```

The same in-memory product passes from materializer to unitizer. Ordinary preparation does not reopen the
retained revision, replay the parser or rebuild a whole expected artifact between adjacent functions.
Persisted/external artifacts are validated once at their acceptance boundary.

## Existing code disposition

| Existing code | Keep / remove |
|---|---|
| [materializer](search-materializer/) | Keep one text/code decoder plus selected Markdown/JATS structure extraction. Remove legacy DIRECT/store/inventory/artifact publication owners and duplicate verification replays. |
| [unitizer](search-unitizer/) | Keep structure-first source occurrences, exact source ranges and one useful prepared encoding. Remove old layout/receipt paths, nested duplicate manifests and discarded whole encodings. |
| [code enricher](search-code-enricher/) | Optional and currently disconnected. Delete unless one real outline/navigation consumer selects one narrow parser profile after baseline search. |

Current raw UnitSet ingress requires byte-identical UTF-8/`ExactBytes`; Markdown/JATS structural paths carry
richer maps/observations. Change one transform and its coordinate/evidence consumers together. A decoder alone
does not establish support for a format.

## Boundaries

- Source bytes and content identity remain with the immutable object owner.
- Prepared artifacts are derived and rebuildable from retained sources plus explicit profile/settings.
- Model/tokenizer packing is a derived projection, not source occurrence identity.
- Exact source revision/range lives in versioned evidence manifests; reusable retrieval/model input is a
  separate identity.
- Text/maps/structure are not duplicated into redb or every unit.
- One immutable writer persists selected prepared bytes; preparation owns no staging, hardlink/rename,
  encryption-key lifecycle, GC or operation receipt.
- Cancellation/limits apply before retaining full outputs. Missing/unsupported transforms remain gaps.

## Structure and reuse

Primary boundaries are document-native:

- Markdown/JATS sections and items;
- code definitions/blocks when one selected syntax outline exists;
- page/region/document elements from an optional selected document profile;
- line/sentence-local fallback only inside an oversized unstructured section.

Split oversized structural units and merge small adjacent peers only within the same heading/context and
selected size bound. Token/model changes rebuild only their projection. Reuse an expensive derived output
only when its exact input, supplied context, model/task and behavior-affecting options match.

## Removal target

Delete after callers move:

- legacy DIRECT preparation/store/inventory codecs and old receipt-bound layout;
- duplicate artifact publication kernels/stages;
- full materializer replay inside unitization;
- raw v3 manifest nested inside Markdown/JATS envelopes when one record set suffices;
- canonical trees/digests/encodings constructed only to be checked and discarded;
- profile/golden/source-shape fixtures for removed pre-release formats;
- disconnected code-fact framework and optional donor programmes without a real consumer.

Child packages may become private modules or merge when no useful dependency boundary remains. Do not create
a preparation service locator, provider registry or new compatibility family.

## Verification

After code: one retained read, one parser pass, exact/mapped coordinates, bounded peak memory, deterministic
current artifact, restart readback or rebuild, no stage leak and no source/Qdrant side effects. Run minimal
Rust 1.98 compiler/Clippy for changed preparation and immediate publication/query consumers; broad format and
optional-enrichment matrices remain late.
