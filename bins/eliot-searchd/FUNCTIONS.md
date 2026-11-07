# Daemon — retained input, publication and read-only search

The daemon owns native invocation, bootstrap settings, root dispatch and factual output. Retention, preparation and publish-prepared use the existing root-owned CLI child; --search-stdio services one framed query with one joinable synchronous worker.

## retain-file path

1. Resolve --source-file against the captured startup directory while preserving native path bytes.
2. Open the existing root with open_plain and retain PlainRootGuard.
3. Capture a read-only current-source guard from the same root control.redb.
4. Call search-safe-reader acquire_file once for the selected path.
5. Supply the acquired buffer as a byte slice to search-revision-store, publish one versioned source ObjectRef beneath guard.object_dir, and read the object back to verify exact bytes, length, and BLAKE3.
6. Commit one CurrentSourceRecord with namespace, random SourceId, NativeFileKey, ObjectRef, and row/registration CAS in the same control.redb.
7. Emit a compact factual JSON result only after the row commit. It reports IDs, revisions, file key, ObjectRef fields, and verified readback; it does not report search evidence.

Retain-file has an 8 MiB bound and does not execute, transform or index the input. Failures are content-free and never become successful empty results.

## prepare-source path

    eliot-searchd --prepare-source --source-id UUID [--unit N] [ROOT] [--data-root ROOT] [--config-file PATH]

native_root_command dispatches to [prepared_source.rs](src/prepared_source.rs). With PlainRootGuard held:

1. Ensure settings exist without replacing an existing selection; capture source, settings and expected outcome from one native read-only CONTROL snapshot.
2. Reuse a Ready artifact matching that input/settings, or read the retained Source ObjectRef and call prepare_utf8. The live locator is never opened.
3. Publish the complete encoded artifact as ObjectKind::Prepared through store_object. Read the stored object and decode_prepared_utf8 before the short guarded outcome commit.
4. Emit only the decoded stored artifact's selected complete text/ranges after CAS succeeds. Empty UTF-8 has an actual zero-unit artifact; binary/invalid UTF-8 and selected ceiling refusals become distinct bound Gaps. Integrity, cancellation, deadline and stale errors remain errors.

Input/output text ceilings are 8 MiB; encoded artifact allowance is 64 MiB, with at most one million lines/units. Execution ceilings are separate from output-affecting PreparationSettings. N defaults to zero. An unavailable ordinal returns PREPARATION_UNIT_OUT_OF_RANGE; it does not create a partial unit. Ready reuse still verifies the addressed artifact; missing/corrupt objects do not trigger rebuilding or live-source fallback.

Preparation stdout is bounded at 397312 bytes for one full 64 KiB unit under worst-case JSON escaping plus metadata/framing. Other root/retention stdout and stderr bounds remain unchanged. Root guard, original deadline/cancellation and child cleanup cover the operation through output.

No operation executes input or implicitly starts a network/model/Qdrant service. Root initialization, inspection, persistent-savepoint safety and incomplete discard are unchanged. [CONTROL](../../crates/search-control-redb/FUNCTIONS.md) owns CAS; [unitization](../../crates/search-prep/search-unitizer/FUNCTIONS.md) owns the complete artifact.

## publish-prepared path

    eliot-searchd --publish-prepared --qdrant-url URL [ROOT] [--data-root ROOT] [--config-file PATH]

[published_corpus.rs](src/published_corpus.rs) holds PlainRootGuard and the original 120-second deadline/cancellation:

1. Capture every current source and matching preparation, including empty/Gap; refuse absent/stale preparation.
2. Persist captured source metadata, compact reference pages and matching evidence in bounded CONTROL writes.
   Hydrate actual upload/readback text from immutable Prepared objects with the captured decoder binding.
   Each pass retains one verified buffer/decoded view across adjacent pages of that source and releases it on
   source change or pass end. Check all unit/range facts and the exact SDK encoded request bytes.
3. Build/verify the vectorless generation through the bridge: InsertOnly/wait, full schema/point/payload/count
   readback and independent source-backed singleton TextQuery canary.
4. Compare/commit current, predecessor cleanup and pending clear atomically; open the completely validated
   matching native RO snapshot before reporting generation state/counts. No writer spans object/backend I/O.

Publication accepts no --query/--limit and returns no user hits. Canary candidates are bounded complete
Unicode alphanumeric/underscore identifiers containing an alphanumeric character, including code/compact JSON.
Exact expected ID/source and finite positive score are required. Empty responses never prove tokenlessness.
Only source-backed absence of alphanumeric characters throughout the actual corpus permits the tokenless case;
no bounded known candidate in otherwise searchable text refuses readiness. True zero-point generations require
complete inventory/evidence/schema and exact count zero, without a sentinel.

The operator owns loopback Qdrant/storage. Existing ELIOT_QDRANT_API_KEY is consumed without logging.
Unknown mutations retain pending targets unserved. See [CONTROL](../../crates/search-control-redb/FUNCTIONS.md#publication-state-and-snapshot)
and [operator storage guidance](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#operator-qdrant-storage).

## Read-only search path

    eliot-searchd --search-stdio --qdrant-url URL [ROOT] [--data-root ROOT] [--config-file PATH]

[framed_search.rs](src/framed_search.rs) services Query/Cancel while one joinable synchronous worker runs
[verified_query.rs](src/verified_query.rs) outside the I/O runtime. The worker opens the root and pin_published,
which does bounded committed-header/table checks without a complete generation scan. It queries the exact
committed collection, preserves backend order and point-resolves only returned evidence plus CapturedSource
from that same native RO snapshot.

Group returned points by captured binding. Verify both Source and Prepared ObjectRefs by actual length/BLAKE3,
decode with the captured source/settings and compare source/prepared bytes plus every selected unit's ordinal,
byte/line range and boundary facts. Emit the exact full UTF-8 unit and citation. A missing returned Source or
Prepared object is an explicit item Gap; corruption/routing/range mismatch is a typed failure. Unreturned source
bodies are not read. A newer current preparation does not replace the pinned capture.

Search never writes CONTROL/collections, publishes, prepares, reconciles or opens live source paths.
Root/view stay held through complete terminal flush; the worker is joined before teardown.
The [protocol](../../crates/search-provider-protocol/FUNCTIONS.md) admits one correlated terminal and caps total
framing at 8 MiB with whole-card OutputLimit. Do not add detached timeout workers or a session framework.

## Verification state

See the [Source recipe](../../crates/search-source/README.md) for the retained-byte/current-row contract and the [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) for exact binary/library revisions, executed native cases, NOT_RUN boundaries and final gate results. Read-only refusal byte preservation must not be extended to a stale writable CAS transaction.
