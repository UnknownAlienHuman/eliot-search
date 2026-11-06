# CLI — one owned daemon operation

The CLI owns local help/version, native argument capture, bounded child output, daemon selection, child completion checks, and closed response validation. Root, retention, preparation and publication use the same one-shot child path.

<a id="capture-and-dispatch"></a>
## Syntax

- eliot-search retain-file --file PATH [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
- eliot-search prepare-source --source-id UUID [--unit N] [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
- eliot-search publish-prepared --qdrant-url URL [--query TEXT] [--limit N] [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
- Existing root operations remain available.

The CLI forwards --file as the daemon's native --source-file argument. Root settings keep their existing precedence and native path handling. It does not open the root or source file itself.

## Retained result

The daemon returns one bounded JSON line only after the source object has been read back and the current source row committed. The CLI validates the operation, random namespace/source IDs, nonzero revisions, source ObjectRef version/kind/digest/length, native file-key shape, and the verified-readback marker before printing it.

Source bytes remain in the immutable object store and are available through read_object. Retention alone does not create preparation or search evidence.

## Prepared result

prepare-source selects an already-retained SourceId. The daemon reads verified retained objects without opening the live source path, stores the complete UTF-8 representation and unit ranges, and commits its guarded current outcome. The response reports the stored artifact, input/settings/revisions and one complete selected unit with text, byte and logical-line ranges. N is a zero-based ordinal, default 0. A real empty artifact has zero units; a bound Gap has a reason and no artifact/unit. Missing or corrupt objects remain errors.

The first argument parse preserves source ID and ordinal for response validation. Never infer them again by scanning forwarded raw arguments: native path values may legally equal --source-id or --unit. The prepared response validator checks against that parsed request.

The preparation stdout allowance is 397312 bytes: six-byte worst-case JSON escaping for a full 64 KiB unit plus 4096 bytes of metadata/framing. It does not shrink unit settings or truncate text. Root/retention stdout and child stderr keep their 64 KiB bounds; owned-child deadline, kill and reap behavior remains.

This is stored text readback, not a query, Qdrant index or citation. See the [daemon recipe](../eliot-searchd/FUNCTIONS.md), [Source recipe](../../crates/search-source/README.md) and [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) for owner boundaries and exact binary/library evidence.

## Publication result

publish-prepared captures all retained sources and their matching preparation outcomes, builds or verifies the exact physical generation, and returns only after current commit and complete RO snapshot validation. The result contains collection/generation, source/Gap/point/page counts and optional real SDK nominations joined to stored point evidence: SourceId/namespace, Source/Prepared ObjectRefs, unit and byte/line ranges. It does not return excerpt text or citations.

TEXT is at most 4096 UTF-8 bytes; N defaults to 10 and is 1..64, with --limit accepted only alongside --query. A deterministic saved-point canary is checked separately before acceptance; an arbitrary user query may legitimately return no hits. No source is truncated to meet a nomination limit.

Only an explicit same-host loopback gRPC URL is accepted; the ordinary local port is 6334. The operator starts Qdrant 1.20.0. An existing optional key is read from ELIOT_QDRANT_API_KEY, never argv/output. Publication stdout has a separate 135168-byte bound: 64 fixed-field evidence records at 2048 bytes each plus 4096 bytes of framing. The CLI validates the closed response against its first parse, without reopening a source or resolving evidence itself.

[Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md#current-q1-consumer) and [CONTROL](../../crates/search-control-redb/FUNCTIONS.md#publication-state-and-snapshot) own readiness/current/evidence behavior. Missing/corrupt current state and backend failure are errors, not fresh or successful empty results. Standalone read-only search with verified text is U1.
