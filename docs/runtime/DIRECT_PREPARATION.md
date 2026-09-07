# Persisted DIRECT preparation

The primary `DirectStore` now uses this write sequence for file/directory ingestion:

```text
verified source snapshot
  -> immutable retained revision, exact readback
  -> shared UTF-8 materializer and unitizer
  -> immutable profile-bound preparation object, exact readback
  -> content-free immutable lookup reference, exact readback
  -> source-catalog event
```

All directory members cross this barrier before any batch catalog event is appended.
An interrupted write can leave an unreferenced object, not a published source whose
new preparation was never written. An unchanged source also verifies/reuses its
preparation. No new runtime process, dependency, search index or mutable owner is added.

## Query and explicit reconstruction

Primary one-shot, persistent and proxy searches use the same store. They reopen the
exact retained revision, verify its length/SHA-256, load the preparation reference and
object, validate the saved line/unit layout, then call the shared cross-unit literal
matcher. They do not materialize a new layout, publish a reference or repair an object.
Layout verification still reads source bytes and validates deterministic boundaries;
this is not a constant-time lookup or a measured performance claim.

Missing, truncated, foreign-profile or conflicting preparation becomes an explicit
source gap and `complete=false`. There is no fallback to query-time preparation.
Unchanged query validation, UTF-8 byte coordinates, LF/CRLF/CR line mapping, overlapping
matches and ASCII-only folding remain. Binary/invalid UTF-8 and deterministic
preparation-limit outcomes are stored as closed gap tags, not successful empty layouts.

Existing roots are not silently backfilled. Prepare a retained revision explicitly:

```text
eliot-searchd --prepare-revision ROOT REVISION_ID
```

This owner-fenced command uses retained bytes, not the current source path. Reindexing
also prepares the current revision. Missing objects/references can be reconstructed;
conflicting existing immutable bytes are refused, never overwritten or deleted.
The existing Windows revision migration on open is unchanged; it is not a preparation
rebuild and is not represented as side-effect-free startup.

## Format and protection

`UnitizationLimits::LAYOUT_FORMAT` is `exact-utf8-line-unit-layout/v1`. Its binary
`ELSLAY01` layout contains the five effective unitization limits, source length,
line/unit counts and ordered line/unit descriptors. It contains no source bodies.
Lengths/counts are checked before allocation; trailing bytes, gaps, overlaps,
noncanonical split points, wrong line flags and invalid UTF-8 boundaries are refused.

The DIRECT profile SHA-256 binds that format, the explicit materializer policy,
materializer/unitizer limits and encoded-byte ceiling. Changing them creates another
lookup/object identity rather than reinterpreting or replacing previous preparation.
The manifest body binds namespace, source ID, retained revision ID, source SHA-256,
source byte length and profile SHA-256 under the `ELSPRP01` envelope.

```text
preparation/refs/<shard>/<lookup-id>.ref
preparation/objects/<shard>/<object-id>.dpapi    # Windows
preparation/objects/<shard>/<object-id>.bin      # explicit development profile
```

The fixed 81-byte `ELSPRF01` reference stores lookup identity, full manifest SHA-256,
length and storage mode. Object identity also binds the manifest bytes and storage
profile. Bodies use the existing namespace/key-bound revision protector on Windows;
only technical references remain plaintext. The existing no-clobber file publication
and exact readback implementation is reused. A conflicting object is never replaced
with fresh ciphertext. Unreferenced partial writes are retained for later maintenance.
Residual preparation blocks fresh-corpus initialization if the authoritative catalog
was lost. At-rest status refuses a blanket encrypted claim when plaintext, malformed
or unexpected preparation objects remain, including a development-to-Windows move.

## Scope

This is persistent preparation for the **existing DIRECT SHA-256 profile**, directly
called by the primary runtime. It is not the full canonical `UnitManifest`/residency
implementation: canonical source occurrence IDs, complete residency closure,
representation/coordinate/assurance receipts and the H5 control-reference migration
remain T13–T16 work. No SHA-256 is cast to BLAKE3, and no fake receipt is issued.
The exact UTF-8 representation borrows the retained revision instead of storing
another copy of its text.

The source catalog still uses its existing append-only format; redb cutover is not
implemented here. These immutable lookup references must move with the canonical
control migration, not become a second long-lived mutable catalog. Existing raw
revision IDs and source-event framing do not change. Older binaries do not consume
these preparation files; downgrade serving is not newly qualified.

Preparation lifecycle graph enumeration, orphan collection, purge/restore and full
cross-platform residency migration remain open. Existing revision GC does not delete
preparation objects. Full Qdrant composition remains separate. Existing tests are
preserved; no new tests, CI changes or execution claim accompany this code increment.
