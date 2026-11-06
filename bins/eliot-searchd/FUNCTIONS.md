# Daemon — root, retained file, preparation and publication

The daemon owns native invocation, bootstrap settings, root dispatch and factual output. Retention, preparation and publish-prepared use the existing root-owned CLI child.

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

    eliot-searchd --publish-prepared --qdrant-url URL [--query TEXT] [--limit N] [ROOT] [--data-root ROOT] [--config-file PATH]

[published_corpus.rs](src/published_corpus.rs) is the concrete serialized publisher and query-nomination consumer. It holds PlainRootGuard and the original 120-second deadline/cancellation through output:

1. Capture every current retained source and matching selected preparation, including empty/Gap. Refuse absent/stale preparation instead of omitting a source.
2. Persist fixed source inputs, full upload text/pages and point/range evidence in bounded CONTROL transactions. Read Prepared objects through the shared verified reader and borrowed decoder; never reopen live locators.
3. Use the [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) for the exact vectorless generation, index, InsertOnly/wait, complete point/payload/count/schema readback and independent saved-point TextQuery canary.
4. Compare/commit current pointer, predecessor cleanup and pending clear atomically; then open the matching complete native RO snapshot. No database writer spans object/backend I/O.
5. Resolve optional SDK hits through that snapshot and return metadata only. Publication success is not an excerpt/citation result.

The query is at most 4096 bytes, default/max hits 10/64, and output at most 135168 bytes. A known zero-point generation has explicit complete inventory/schema/count checks. Nonzero populations require a whole saved ASCII-word candidate and exact singleton ID/source/positive-score canary; no known candidate or unexpected empty response refuses acceptance. Optional user queries run independently and may return zero.

The operator owns the loopback Qdrant service/storage. The optional existing ELIOT_QDRANT_API_KEY is consumed without logging. Unknown mutations retain an unserved pending target; do not clear them from a matching readback or reset the failed root. See [CONTROL](../../crates/search-control-redb/FUNCTIONS.md#publication-state-and-snapshot) for whole-graph validation and guards, and [root guidance](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#operator-qdrant-storage) for the observed Windows storage constraint.

## Verification state

See the [Source recipe](../../crates/search-source/README.md) for the retained-byte/current-row contract and the [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) for exact binary/library revisions, executed native cases, NOT_RUN boundaries and final gate results. Read-only refusal byte preservation must not be extended to a stale writable CAS transaction.
