# CLI — one owned daemon operation

The CLI owns local help/version, native argument capture, daemon selection, owned-child completion and safe bounded output. Root, retention, preparation and publication retain their argv operation path; search uses the serviced framed query child.

<a id="capture-and-dispatch"></a>
## Index and refresh

    eliot-search index [PATH] --qdrant-url URL [--data-root ROOT] [--config-file PATH] [--daemon PATH]

Initialize a new root once with initialize-data-root. PATH admits and refreshes only that explicit File/Tree;
bare index reloads all stored admissions in the new process and refreshes the unique union. Explicit File wins
over Tree ignores; overlap is processed once. Admission/kind persist through deletion/re-add. Bare index with
no admissions returns INDEX_SELECTION_REQUIRED; old retain-file rows are not inferred admissions.
The owned child uses one guard/deadline and typed retain/prepare/publication helpers. Only confirmed absence
removes current membership; an ignored still-present source remains. Link/incomplete observations fail.
Unrelated unprepared current rows still cause PUBLICATION_SOURCE_NOT_PREPARED. Success is the closed
index/refreshed result; use search for cards. Unchanged key/ObjectRef preserves logical revisions/collection.

## Syntax

- eliot-search retain-file --file PATH [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
- eliot-search prepare-source --source-id UUID [--unit N] [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
- eliot-search publish-prepared --qdrant-url URL [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
- eliot-search search --query TEXT [--limit N] --qdrant-url URL [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
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

publish-prepared captures all retained sources and matching preparation outcomes, builds or verifies the fixed
physical generation and reports only after current commit and complete publication/recovery snapshot validation.
The closed JSON response contains operation/state, collection/generation and source/Gap/point/page counts.
Publication accepts no --query/--limit and returns no user hits. The independent saved-point canary remains.

Only an explicit same-host loopback gRPC URL is accepted; the ordinary local port is 6334. The operator starts
Qdrant 1.20.0. An existing optional key comes from ELIOT_QDRANT_API_KEY, never argv/output. The existing 135168-byte
publication stdout cap is a bound, not a requirement to fill it or return nominations.
[Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md#current-q1-consumer) and
[CONTROL](../../crates/search-control-redb/FUNCTIONS.md#publication-state-and-snapshot) own readiness/current/evidence.
Backend/current failures remain errors, not successful empty results.

## Read-only search

Search is the sole user query route. Its first parse preserves query/limit and native child settings.
TEXT is 1..4096 UTF-8 bytes without NUL; N defaults to 10 and is 1..64. The CLI constructs QueryClient outside
an async context, enables its Ctrl-C listener before spawn and sends SearchRequest to --search-stdio with
remaining duration sampled at send. It prints only a complete validated correlated terminal after owned-child
exit/reap; untrusted text/diagnostics pass through safe JSON presentation.

A hit contains the full stored unit (at most 64 KiB), original rank/finite score and the immutable citation:
generation/point, source namespace/ID, input/prepared ObjectRefs, unit and byte/line ranges/boundary facts.
Missing returned objects are explicit gaps; integrity/routing errors fail. Newer preparation cannot replace
the published binding. Search does not publish, reprepare, scan the source corpus or open live paths.

The [protocol](../../crates/search-provider-protocol/FUNCTIONS.md#framing-and-bounded-output) caps the entire
u32-LE/JSON terminal at 8 MiB and emits only whole cards. OutputLimit reports an omitted tail without truncating
unit text or inventing completeness. The [client](../../crates/search-provider-client/FUNCTIONS.md) owns serviced
I/O, cancellation and one observed child wait/reap. Publication's argv output allowance is not this frame budget.
