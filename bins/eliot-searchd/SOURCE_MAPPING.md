# Persisted source-migration mapping draft

The running DIRECT service accepts:

```text
control-migration-plan<TAB>TARGET_NAMESPACE_UUID
```

`<TAB>` is one literal tab. The target must be an explicit, non-nil canonical UUID.
It names the proposed imported namespace, not an existing native identity. This command
writes a reusable mapping artifact; it does **not** import into redb or switch authority.

## Mapping

The existing full source-journal replay drives `compile_source_mapping`. Each input event
produces exactly one row retaining the original operation, source, content-object and
predecessor identities. The output uses the existing `SourceId`/`SourceRevisionId` types.

A first activation, changed content, or reactivation opens a revision occurrence. An
active path-only change retains the current occurrence; retirement closes source activity
without allocating another revision. Thus `A -> B -> A` has three occurrences even though
the legacy object inventory contains only two distinct content objects. Source event
ordinals and global event sequence remain separate from the occurrence sequence.

Proposed imported IDs use the existing SHA-256 multipart framing and UUID version-8 bits:

- source: domain `eliot-search/imported-source-uuid/v1`; target namespace bytes, legacy
  namespace bytes, legacy source ID ASCII;
- revision: domain `eliot-search/imported-revision-uuid/v1`; target namespace bytes, mapped
  source UUID bytes, legacy event digest ASCII, occurrence sequence as big-endian u64.

Take the first 16 digest bytes, set byte 6 high nibble to 8 and byte 8 high bits to `10`.
The complete derivation digest is retained during compilation to reject shortened-ID
collisions. These IDs describe imported lineage; no legacy digest becomes an NTFS file ID.
Legacy SHA-256 values remain labelled SHA-256. No BLAKE3 value is synthesized.

## Artifact and retry

The artifact is newline-delimited JSON: one `source_mapping_header`, all
`source_event_mapping` rows, then one `source_mapping_end`. It contains technical
identifiers and counters, not source bodies, path text, credentials or access grants.
The explicit mapping profile and complete logical source-chain snapshot bind its header.

The writer streams into a create-new temporary file, syncs, recompiles against a second
complete validated replay and compares every byte and final counts. Only then is the file
published without replacement under `control/migration-plans/<digest>.source-map.v1`.
An existing destination must pass fingerprint and length readback. A failed command never
deletes a published plan. Retrying the same target and unchanged history produces the
same artifact; older plans do not authorize importing a changed source snapshot.

`plan_chain_sha256` is **not raw-file SHA-256**. Its scheme is `sha256-record-chain-v1`:
seed with multipart domain `eliot-search/source-map-chain/v1`; fold each complete row
(including LF) with `eliot-search/source-map-row/v1`, prior chain and 1-based u64 row number;
finalize with `eliot-search/source-map-end/v1`, final chain, row count and byte count.
All integers in the hash input are big-endian u64. Original hash helpers are unchanged.

Limits: 8 KiB per encoded row, 512 MiB per artifact, existing source-event bounds and a
cooperative 120-second operation deadline. Synchronous filesystem calls are not preempted;
transport deadlines may be shorter. Interrupted work may leave a temporary or completed
artifact, never an implicitly accepted import. Post-dispatch failures use the existing
service mutation-outcome-unknown handling. The plan is not read by normal search.

## Remaining import inputs

The mapping draft cannot construct a complete `SourceRevision`: migration still needs
actual BLAKE3 readback, an import observation/stability receipt, explicit residency policy,
root/workspace/membership associations and the namespace owner-cutover protocol. Legacy
observation times are unknown, not replaced with fabricated historical timestamps. Root
registration and filenames cannot supply missing admission or access authority.

See `docs/contracts/p00/SOURCE_GRAPH.md` for the normative graph. This draft neither accepts
T10 nor implements T11 cutover. Compilation and execution of this increment are unverified.
