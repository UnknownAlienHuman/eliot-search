# Persisted source-migration mapping draft

The running DIRECT service accepts:

```text
control-migration-plan<TAB>TARGET_NAMESPACE_UUID
```

`<TAB>` is one literal tab. The target must be an explicit, non-nil canonical UUID.
It names the proposed imported namespace, not an existing native identity. This command
writes the reusable mapping artifact, verified content-digest manifest and a populated
**inactive redb source-map database**. It does not activate that database or replace the
working control journal.

## Offline source-preserving entry

```text
eliot-searchd --plan-control-migration ROOT TARGET_NAMESPACE_UUID OUTPUT_DIRECTORY
```

Both directories must already exist. The output must be outside the data root and every
registered observation root; neither ancestor nor descendant overlap is allowed. The reply's
`plan_locator` is relative to `OUTPUT_DIRECTORY` when `plan_location=explicit_output_directory`.
The live-service reply uses `plan_location=data_root` and the previous root-relative locator.

The offline command runs before ordinary daemon startup. It opens the existing
`.eliot-search-owner.lock` and holds the same exclusive OS lock throughout inspection and
publication. An active service blocks the command. A missing lock is an explicit recovery
requirement: no replacement lock file or owner record is created. The lock locator's native
file identity is rechecked before and after work on Windows/Unix, and no normal owner-marker
cleanup runs. This is process exclusion, not a namespace cutover or power-loss qualification.

Existing namespace/source-history records use the same full replay and mapping compiler.
Retained revision objects are then read and verified; current source paths are never reopened.
Root registration is read without recovery and compared again before the reply. Pending
registration updates, missing catalog files and malformed history fail without repair.
No normal DirectStore initializer, credential creation, plaintext-to-protected conversion,
current-source read or revision/preparation writer is invoked. Windows protected objects use
only their existing namespace credential; a missing key is never created to satisfy an import.
Only the explicitly selected output directory receives temporary/published artifacts;
filesystem read-access metadata is not claimed immutable. Interrupted work can leave an inert draft.

Source-map bytes, IDs, profile and fingerprint are identical to live-service mode for the
same target and history. Existing files are not replaced, and lost output acknowledgement
can be retried. The planner verifies payload digests but does not copy payloads into redb.
Other inventory commands retain their existing already-open-service prerequisites.

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
published without replacement as `<digest>.source-map.v1` in the selected output directory
(`control/migration-plans/` for the live service). An existing destination must pass fingerprint
and length readback. The source chain is revalidated before acknowledgement. A failed command
never deletes a published plan. Retrying the same target and unchanged history produces the
same artifact; older plans do not authorize importing a changed source snapshot.

`plan_chain_sha256` is **not raw-file SHA-256**. Its scheme is `sha256-record-chain-v1`:
seed with multipart domain `eliot-search/source-map-chain/v1`; fold each complete row
(including LF) with `eliot-search/source-map-row/v1`, prior chain and 1-based u64 row number;
finalize with `eliot-search/source-map-end/v1`, final chain, row count and byte count.
All integers in the hash input are big-endian u64. Original hash helpers are unchanged.

Limits: 8 KiB per encoded row, 512 MiB per artifact, existing source-event bounds and a
cooperative 120-second operation deadline. Synchronous filesystem calls are not preempted;
transport deadlines may be shorter. Interrupted work may leave a temporary or completed
artifact, never an implicitly accepted import. Post-dispatch service failures use the existing
mutation-outcome-unknown handling. The offline command exits on failure; lost output confirmation
is `DIRECT_MIGRATION_PLAN_ACK_OUTCOME_UNKNOWN`. The plan is not read by normal search.

## Canonical content-digest manifest

Both planner entrypoints also produce `<chain>.source-content.v1`. Its header binds the
explicit target namespace, original namespace, exact catalog snapshot, source-map record chain,
content profile and expected retained-object count. Each ordered `source_content_readback` row
contains the legacy source/revision IDs, original SHA-256, byte length and newly computed
`content_blake3`. The footer accounts for all objects and bytes. These are content-object facts,
not extra revision occurrences: multiple mapped occurrences may reference one retained object.

The existing revision reader validates identity, length and SHA-256 before BLAKE3 sees the bytes.
The digest is ordinary unkeyed BLAKE3-256 over exact bytes, including empty/binary content; no
UTF-8 conversion, normalization or materialization is involved. The writer retains at most one
revision body at a time and updates the zeroizing BLAKE3 hasher in 256 KiB chunks with deadline
checks. Each body is bounded by the existing 64 MiB source limit.

A second complete pass reopens the retained objects, recomputes every digest and compares the
exact manifest bytes and final counts before no-clobber publication and final-file readback.
The same 120-second budget spans source mapping, both content passes and redb staging. A missing
or contradictory revision rejects the command; the output never claims a skipped object verified.
Published inert artifacts from earlier steps can survive failure and be verified/reused on retry.

Explicit migration may inspect an old plaintext `.bin` on Windows without generating a key or
rewriting the source. Ordinary DIRECT serving still requires protected Windows objects. If a
protected copy exists, it must decrypt with the original credential and agree with any plaintext
copy; an invalid protected copy never falls back. This is one shared reader with explicit policy,
not a second content decoder or an alternative serving route.

`content_manifest_locator` uses `plan_location`; `content_manifest_chain_sha256` uses the same
explicit record-chain scheme described above, not raw-file SHA-256 or a BLAKE3 tree over the
manifest. The header's distinct schema/profile prevents confusing it with source mapping.
`content_blake3_verified=true` acknowledges both content passes, not stability/residency admission.
The inactive redb schema and source-map v1 bytes remain unchanged. A future canonical importer
must consume this manifest with its exact source-plan binding; it is not silently inserted as
active H5 state or accepted as a namespace-cutover receipt.

### Hash implementation pin

Composition uses `blake3 = 1.8.2`, default features disabled, with `std`, `pure`, `zeroize`.
This is an explicit implementation pin, not a latest-version or qualification claim. Upstream
`BLAKE3-team/BLAKE3` tag `1.8.2`, `Cargo.toml`, `build.rs`, and `src/lib.rs` define this profile:
`pure` selects Rust hashing implementations, and `zeroize` covers the hasher state. No mmap,
Rayon, C/assembly hashing implementation, helper process or Python runtime is enabled.
The Rust `cc` crate remains an upstream build-script dependency; it is not a runtime dependency.

Checksums/dependency requirements were read from the corresponding `rust-lang/crates.io-index`
entries. New lock entries are `blake3 1.8.2`, `arrayref 0.3.9`, `arrayvec 0.7.6`,
`constant_time_eq 0.3.1`, `cc 1.1.12`, `shlex 1.3.0`; existing packages are not upgraded.
BLAKE3 registry checksum: `3888aaa89e4b2a40fca9848e400f6a658a5a3978de7be858e209cafa8be9a4a0`.
The lockfile delta was prepared from those exact entries; Cargo resolution and execution have
not been run here. Internal package dependencies and external artifact qualification are unchanged.

## Inactive redb source-map import

Both existing entrypoints also produce `<digest>.source-map.v1.redb` beside the text plan.
`staged_database_locator` uses the same location scope as `plan_locator`. This is a real
redb 2.6.3 file, built by `search-control-redb::migration`; vendor handles stay inside
that adapter. Its four `eliot.import.source-map.*.v1` tables contain binding/progress,
ordered technical event rows, latest-event references by source ID, and first-event
references by occurrence ID. They have no source bodies, paths, vectors, tokens or policies.
This schema is intentionally not accepted by `PersistentControlJournal` as an active journal.

The existing compiler delivers typed rows directly to the importer. No JSON is interpreted
as trusted metadata. Each batch of at most 256 rows atomically commits events, index links
and progress with immediate durability. Global and per-source predecessors, occurrence
transitions, duplicate IDs and counts are checked before a completion marker can be stored.
The adapter admits at most 2,000,000 events and 512 MiB of fixed 371-byte event values;
these are logical limits, not a byte-exact maximum for the native redb file. Cache is 8 MiB.
All steps share the caller's cooperative deadline rather than starting a fresh budget.

The temporary target is closed and reopened. A complete compiler replay then compares every
binary row and every source/occurrence index entry against the verified source history in
one coherent read transaction. A prefix, surplus row, wrong index, different target/profile,
foreign plan or incomplete marker fails. The final no-clobber publication is checked again.
Existing completed targets are verified and reused; conflicting targets are never replaced.
A failed import can leave its already published text plan. Retrying fills in the missing
sidecar, or verifies an existing completed one. Partial temporary targets are not resumed
as accepted progress: they are discarded/rebuilt, and process death may leave temporary residue.
Native redb recovery may modify the staging target, never the source root in offline mode.

`source_mapping_imported_to_redb=true` and `staged_database_verified=true` describe this
source-mapping scope only. `redb_imported=false`, `active_control_imported=false` and
`canonical_records_materialized=false` remain explicit until the complete H5/control import
exists. Sealing this artifact is not a cutover receipt, security grant, source admission,
current-workspace proof or product-readiness result. Rust execution remains unverified.

## Remaining import inputs

The mapping draft alone cannot construct a complete `SourceRevision`. The content manifest now
supplies actual BLAKE3 readback, but canonical import still needs an import observation/stability
receipt, explicit residency policy, root/workspace/membership associations and the namespace
owner-cutover protocol. Legacy observation times are unknown, not replaced with fabricated
historical timestamps. Root registration and filenames cannot supply missing admission or access
authority. The older source-map v1 footer lists its missing inputs; the separately bound content
manifest supplies its content-digest input without changing that format.

See `docs/contracts/p00/SOURCE_GRAPH.md` for the normative graph. This draft neither accepts
T10 nor implements T11 cutover. Compilation and execution of these paths are unverified.
