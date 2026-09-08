# Source-migration artifacts

## Entry points

```text
control-migration-plan<TAB>TARGET_NAMESPACE_UUID
eliot-searchd --plan-control-migration ROOT TARGET_NAMESPACE_UUID OUTPUT_DIRECTORY
```

The first command runs in the existing DIRECT service. The second runs before ordinary
startup. Both require an explicit non-nil target namespace UUID and produce the same
source mapping, verified content manifest and **inactive** redb target. They do not replace
the working catalog, issue admission/residency grants or activate a namespace.

The offline directories must already exist. The output cannot overlap the data root or
any registered observation root in either ancestor direction. It opens and exclusively
locks the existing `.eliot-search-owner.lock`, rechecks its native identity before/after
work and does not rewrite or clean its owner marker. A missing lock requires recovery;
an active service blocks offline planning. Registration is reread without recovery.

No normal DirectStore initializer, credential creation, current-source read, root repair,
plaintext conversion or revision/preparation writer runs in offline mode. Only the selected
output directory receives artifacts. Filesystem read-access metadata may change.
`plan_location=explicit_output_directory` makes returned locators relative to that directory;
`plan_location=data_root` uses `control/migration-plans/` for the service.

## Source occurrences and `.source-map.v1`

The shared full journal replay and compiler emit one `source_mapping_header`, exactly one
`source_event_mapping` per original event and one `source_mapping_end`, as UTF-8 NDJSON.
All original operation, predecessor, source, object and path fingerprints are retained.
No source bodies, path text or secrets enter the artifact.

First activation, content change and reactivation create new revision occurrences.
An active path-only change retains its occurrence; retirement closes activity without
allocating a revision. `A -> B -> A` therefore has three occurrences but may use two
retained content objects. Global event sequence, per-source event ordinal and per-source
occurrence sequence remain distinct.

Proposed UUIDv8 IDs use the existing SHA-256 multipart framing:

- Source domain `eliot-search/imported-source-uuid/v1`: target namespace bytes,
  legacy namespace bytes, legacy source-ID ASCII.
- Revision domain `eliot-search/imported-revision-uuid/v1`: target namespace bytes,
  mapped source UUID bytes, legacy event-digest ASCII, occurrence as big-endian u64.

Take the first 16 digest bytes, set byte 6 high nibble to 8 and byte 8 high bits to `10`.
The compiler retains full derivations to reject shortened-ID collisions. These are imported
lineage identities, not fabricated NTFS identities. SHA-256 is never relabelled as BLAKE3.

The source map is streamed to a create-new temporary file, synced, recompiled from a second
validated replay and compared byte-for-byte, including final counts. Publication uses a
no-clobber hard link followed by final readback. Existing conflicting artifacts are not
replaced. Its name remains `<source-plan-chain>.source-map.v1`.

## Record-chain identity

`sha256-record-chain-v1` is **not raw-file SHA-256**. Both NDJSON artifacts use it:

1. Seed with multipart domain `eliot-search/source-map-chain/v1`, no parts.
2. Fold each complete LF-terminated row with domain `eliot-search/source-map-row/v1`,
   prior chain, 1-based row number as big-endian u64 and exact row bytes including LF.
3. Finalize with `eliot-search/source-map-end/v1`, chain, row count and byte count,
   both counters as big-endian u64.

Their distinct schema/profile headers prevent confusing source mapping with content facts.
Limits are 8 KiB per row, 512 MiB per artifact and one cooperative 120-second budget across
all source replays, content reads and target writes/readbacks. Blocking OS/redb calls are
not preempted; transport deadlines may be shorter. Post-dispatch service failures remain
mutation-outcome-unknown. Lost offline output is `DIRECT_MIGRATION_PLAN_ACK_OUTCOME_UNKNOWN`.

## Verified content and `.source-content.v1`

The content header binds target namespace, legacy namespace, exact catalog snapshot,
source-plan chain, content profile and expected retained-object count. Ordered
`source_content_readback` rows contain legacy source/revision IDs, original content SHA-256,
byte length and actual `content_blake3`. The footer accounts for all objects and source bytes.
These are object facts, not additional source occurrences.

The shared reader first validates legacy identity, length and SHA-256. Ordinary unkeyed
BLAKE3-256 is then computed from exact retained bytes, including empty and binary objects,
without UTF-8 conversion, normalization or materialization. One zeroizing revision buffer
and hasher are retained at a time; hashing checks deadlines every 256 KiB. Each body is
bounded by the existing 64 MiB limit.

A second full pass reopens every object, recomputes its digest and compares all encoded
manifest bytes and counts. No missing object becomes a successful skipped row. Only then
is `<content-chain>.source-content.v1` published without replacement and read back.

On Windows, explicit import may inspect old plaintext without conversion or key creation.
A present protected copy must authenticate using the existing namespace credential and
agree with any plaintext copy; it never falls back after a decryption failure. Normal
DIRECT serving still requires protected Windows objects. These are explicit policies of
one shared reader, not alternate serving routes.

## Content-bound inactive redb target

The planner now creates `<target-digest>.source-map.v2.redb`. Derive `target-digest` using
existing multipart SHA-256 domain `eliot-search/source-content-import/v2` with source-plan
chain bytes and content-manifest chain bytes. A different content profile/manifest cannot
reuse an older pending target. Text source-map v1 and content-manifest v1 encodings do not change.

The four `eliot.import.source-map.*.v1` tables retain their existing row/index codecs:
metadata, ordered events, latest source-event references and occurrence-opening references.
The v2 envelope additionally requires META `content_manifest`, a fixed 208-byte reference:

```text
ELSCREF1 (8 bytes)
target namespace UUID (16 bytes)
legacy namespace SHA-256 (32 bytes)
catalog snapshot SHA-256 (32 bytes)
source-plan record chain SHA-256 (32 bytes)
content profile SHA-256 (32 bytes)
content-manifest record chain SHA-256 (32 bytes)
manifest bytes, object count, total source bytes (3 big-endian u64)
```

This reference must exactly match the source-map binding and producer result. Manifest
length/count bounds are checked before initialization; objects must lie between source
and occurrence counts when sealing. Bodies and large digest inventories remain in their
original stores/immutable manifest, not in redb values. The declaration alone is not
proof of source readback: the daemon obtains it from the two verified content passes.

Creation stores binding, content reference and empty progress atomically. Every subsequent
batch, sealing transaction and reopen compares the exact reference. Source-only APIs reject
v2 metadata; content-bound APIs reject an absent/foreign reference. No code adds a reference
to an existing v1 target. Existing v1 targets and their pending files remain untouched.

Both original planner entrypoints call the content-bound adapter. The content artifact's
exact length and record chain are rechecked before redb work and after final target readback,
including the existing-target reuse path. No JSON parser converts caller strings to authority.
The reply identifies `staged_database_schema=source-map-content-v2` and
`content_manifest_bound_to_redb=true` only after these steps succeed.

## Batches, resume and final readback

At most 256 typed events are buffered. Each immediate-durability transaction commits rows,
source/occurrence indices and all five progress counts atomically. The adapter checks global
and per-source predecessors, occurrence transitions, duplicate identities and counts.
Bounds remain 2,000,000 events, 512 MiB of fixed 371-byte event values and an 8 MiB cache;
this is not a byte-exact maximum for the native redb file.

Failure preserves `.<target-digest>.source-map.v2.redb.pending`. Repeating the same operation
reopens the same binding and manifest. The compiler restarts at event one and compares the
entire committed prefix and indices in one read transaction before any suffix write.
Verified rows are neither rewritten nor counted twice. Uncommitted buffered rows are replayed.
Sealed targets receive no new application writes. Resume saves writes, not full validation cost.

Corrupt progress, missing tables, empty files, mismatched bindings or altered rows fail without
deleting, replacing or rebuilding the pending database. Changed input selects a different name.
Completed targets are reopened and every row/index is compared with a fresh complete compiler
replay; a prefix, extra row or wrong index cannot pass. Final publication is no-clobber and is
verified again. The pending name is removed only after success. A crash after publication may
leave both names; random old temporary names are never guessed or adopted. Native redb recovery
may modify the staging target, not source files in offline mode.

## Implementation pin and limits of this result

The existing `blake3 = 1.8.2` pin disables defaults and enables `std`, `pure`, `zeroize`.
No mmap, Rayon, C/assembly hashing or helper runtime is enabled; `cc` remains an upstream
build-script dependency. Registry checksum is
`3888aaa89e4b2a40fca9848e400f6a658a5a3978de7be858e209cafa8be9a4a0`.
The pinned lock additions were `arrayref 0.3.9`, `arrayvec 0.7.6`, `constant_time_eq 0.3.1`,
`cc 1.1.12`, `shlex 1.3.0`. This binding change adds no dependencies or upgrades.

`source_mapping_imported_to_redb`, `staged_database_verified`, `content_blake3_verified`
and `content_manifest_bound_to_redb` describe only the implemented artifact scope.
`redb_imported=false`, `active_control_imported=false`, `canonical_records_materialized=false`
and `cutover_authorized=false` remain explicit. The target is not accepted as an active
`PersistentControlJournal` and is not consulted by ordinary search.

Canonical import still needs observation/stability evidence, explicit residency policy,
root/workspace/membership associations and namespace owner cutover. Unknown historical times
are not invented; registration and filenames do not grant access. Source-map v1's missing-input
footer is unchanged; the bound content manifest supplies its content-digest input.
See `docs/contracts/p00/SOURCE_GRAPH.md`. T10/T11 and runtime qualification are not claimed
complete. Compilation and execution remain unverified.
