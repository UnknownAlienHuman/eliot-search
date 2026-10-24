# Working on source evidence resolution

Read the [architecture](../../../docs/architecture/README.md),
[query design](../README.md), [README](README.md) and [function map](FUNCTIONS.md).

Implement one direct `point_id → pinned ViewManifestEntry → retained bytes/range` path. The view and source
owners provide expectations; a Qdrant payload or caller nomination cannot choose source truth.

Use the actual immutable-object reader and request deadline/budget. Verify object identity/content at the
real storage boundary, then validate the requested range/coordinate mapping. Do not accept echoed digest,
profile, residency or assurance fields and do not rehash the entire object once per excerpt if the reader
already verifies it.

Classify a point absent/contradictory in the pinned view as backend/leg inconsistency; classify a genuinely
missing retained artifact/range as a candidate gap. Preserve final rank and source-backed evidence. No
grant/checkpoint/permit is required for the owning local client; restricted-session cancellation stays at
its real boundary.

Change actual executor/manifest/reader/result consumers together. Remove public nomination/context/readback
proof structures, per-call budget resets and fallback identities. If only a small lookup adapter remains,
inline it rather than preserving this package for fixtures.
