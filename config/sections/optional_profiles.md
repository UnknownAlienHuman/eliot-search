# Optional capabilities and settings

Optional features use the same source, preparation, publication and query path. Select a concrete
extractor/model and its real settings when implementing that feature; a profile receipt is not runtime
configuration. Follow the [implementation guide](../../docs/implementation/README.md).

## Current source

The daemon [registry](../../bins/eliot-searchd/src/config_composition/kernel/registry.rs) registers only
`optional_profiles.semantic`, default `false`, with the old `GateRequired` action.
[Readiness](../../bins/eliot-searchd/src/config_composition/kernel/readiness.rs) consults that flag and a
supplied acceptance value. This is not a connected model runtime. `document`, `advanced_scale` and the
three old profile-reference fields in the historical configuration catalogue are not implemented fields
of this registry. Both the doc worker and model worker exit unavailable; the
[model-provider directory](../../crates/search-model-provider/README.md) has no implementation.

Do not advertise those catalogue fields as working toggles. Replace the actual registry, typed settings,
runtime construction and status together as a capability is connected. Remove P15/G6 acceptance digests
and generic migration/removal receipts from the ordinary settings path.

## Apply the change the capability needs

| Capability | Concrete settings and effect |
|---|---|
| Native BM25 | Normal local Qdrant lexical configuration; it is not optional model-worker inference. Change indexing/query options together and rebuild affected derived vectors when their encoding changes. |
| PDF/document extraction | Selected parser, supported formats and real limits; [worker implementation](../../bins/eliot-search-doc-worker/FUNCTIONS.md). Reprepare changed extractor outputs. Adding a format does not inherently require rebuilding unrelated corpora or changing the Qdrant collection schema. |
| Reranking | Selected local model/runtime, candidate cap and deadline. Reranking already retrieved candidates needs no persistent vector rebuild. |
| Dense/multivector retrieval | Selected model, vector name/dimensions and encoder settings. Publish compatible derived vectors/schema before advertising retrieval; never reinterpret the old vectors. |
| More workers/shards | Actual concurrency/topology settings for a measured bottleneck. A generic `advanced_scale` boolean does not implement capacity. Keep the one-shard local baseline until it needs to change. |

Reuse model outputs with existing prepared artifacts by exact input/context and actual model/task/options.
A changed source occurrence gets its current revision/range; an old vector never carries forward source
authority. Reranking needs no corpus graph, LLM query rewriting or generated-answer service.

Disabled or unavailable extensions leave ordinary text/code search usable and report a specific gap.
Stop optional work and release its processes/resources when disabled. Keep live query pins valid while
retiring derived data. Concrete reprocessing or restart may be required; a universal rollback/migration
platform is not. Configuration must not silently download models or start linked-resource crawling.
