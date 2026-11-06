# Unitization and the complete UTF-8 prepared artifact

The selected P1 implementation exposes prepare_utf8 and decode_prepared_utf8 from [src/prepared.rs](src/prepared.rs).
It depends on the exact owned [materializer product](../search-materializer/FUNCTIONS.md), registry settings/source
values and maintained BLAKE3. It owns no file writer, CONTROL transaction or backend point lifetime.

## Baseline output

Reuse line-preferred range selection over the same MaterializedText text and line inventory. There is no copied
line inventory or per-unit String. Oversized lines split only at UTF-8 scalar boundaries; a scalar larger than
the selected hard unit size is an explicit UnitTooLarge refusal. Complete empty text produces zero units.

UnitSpan describes exact source/identical representation [start,end), logical-line bounds and complete-line flags.
All units are text under this fixed artifact format. Ordinal is vector position within the immutable artifact,
not cross-revision continuity or a backend point ID.

## Representation and ownership

PreparationSettings is registry-owned: utf8_version, line_unitizer_version, preferred_unit_bytes and max_unit_bytes.
The supported pair is 1/1 with 0 < preferred <= hard <= the registry's 64 KiB complete-unit maximum.
PreparationLimits separately carries fail-only input/output/line/unit/encoded ceilings. Changing those ceilings
cannot change successful encoded bytes.

prepare_utf8 consumes the verified input Vec through String::from_utf8, computes ranges against its attached
inventory and encodes the text plus precomputed descriptors once. Hashing, scanning, allocation, descriptor and
encoding work check the original deadline/cancellation flag and finite dimensions. Size arithmetic is checked;
bounded allocations are fallible. The encoded-byte ceiling is checked before reserving the complete output.

The shared object writer publishes and independently verifies the returned Vec as ObjectKind::Prepared.
The artifact never contains its own subsequently returned ObjectRef. Only the current-preparation outcome and
later evidence records bind that reference.

## Saved wire and acceptance

The little-endian complete artifact has magic ELSPRP01 and codec version 1, followed by:
source namespace/SourceId, registration revision, exact input ObjectRef format/kind/digest/length,
the typed settings tuple, representation length, descriptor count, actual representation bytes, then descriptors.
Each descriptor encodes four u64 coordinates and two strict boolean bytes. Header and total sizes are derived
from those actual fields with checked arithmetic; no nested legacy/layout-only encoding is retained.

Source row_revision and CONTROL settings/outcome CAS revisions are excluded from artifact identity.
The decoder checks expected source/input/registration/settings, source-only input kind, supported framing,
actual representation length/BLAKE3, UTF-8, finite total size/counts, contiguous complete coverage, scalar boundaries,
hard unit size and real logical-line facts. Counts/total length are checked before descriptor allocation.
Line facts are checked with a streaming cursor, without constructing another line inventory or recomputing
preferred unit boundaries. Trailing bytes and noncanonical boolean values are refused.

PreparedUtf8 has private borrowed text and a bounded descriptor Vec, exposed through text() and units().
Only the artifact buffer determines its lifetime; expected source/settings are checked, not borrowed into it.
The real daemon prepare-source consumer reads its stored Prepared object and uses that borrowed view to emit
bounded actual unit text/ranges. JSON serialization allowance belongs to that caller, not altered chunk settings.

## Empty and gap semantics

Supported empty text creates a complete saved artifact with zero units and exact source/input/settings binding.
BinaryContent/InvalidUtf8 and selected fail-only ceiling refusals can become distinct bound current Gaps.
InvalidSettings/InvalidLimits, binding/integrity/artifact failures, allocation/overflow, Cancelled and Deadline
remain actual operation errors. No sentinel, fabricated empty success or complete-looking partial result exists.

## Current outcome and later consumers

The daemon captures source/current settings/expected outcome, completes object I/O outside redb, then compares
the full captured guard atomically. Stale success/failure cannot install across row changes, ABA or remove-add.
The first actual consumer is prepare-source --source-id UUID --data-root ROOT, using verified retained bytes
even if the original path changes or disappears. retain-file does not acquire an automatic preparation route.

Q1 will consume the stored artifact when its actual publication path is implemented. The old receipt unitizer,
v3 manifest, Markdown/JATS unitsets and layout-only codec are physically removed with exclusive fixtures.
Their obsolete external projection/publication interfaces are retired by the integration owner. No future
consumer name justifies preserving those routes.

## Verification

After the designated lock-resolution handoff, run the smallest locked check/strict Clippy for both prep owners
and real consumers. Focused saved-artifact cases cover exact text/ranges, borrowed storage, empty, source/registration/
settings binding, ceiling-independent identity, corrupt counts/ranges/flags/trailing bytes, integrity and transient
execution failures. Runtime qualification of shared object/CONTROL/CLI behavior belongs to its designated verifier;
this recipe does not assert those commands have passed.