# Source and preparation values

## Current types

| Type | Responsibility |
|---|---|
| CurrentSourceRecord | Persisted current source identity, locator, native file key, ObjectRef, row revision, and registration revision. |
| ObjectRef / ObjectKind | Version 1, Source (tag 1) or Prepared (tag 2), maintained BLAKE3 and exact byte length. Source rows accept only Source references. |
| NativeFileKey | Exact Unix device/inode or Windows volume/file-index pair. Unsupported native IDs are not replaced with path or digest guesses. |
| PreparationSettings | Output-affecting UTF-8/unitizer versions and preferred/hard unit byte sizes, shared by CONTROL and unitizer. |

search-control-redb owns table definitions, random ID creation, locator lookup, row encoding, and the atomic CAS. This crate supplies no registry, journal, compatibility codec, admission receipt, history, or replay API.

Supported preparation versions are 1/1, with 0 < preferred_unit_bytes <= max_unit_bytes <= 65536. Defaults are 16384/65536 bytes. Fail-only execution ceilings belong to PreparationLimits in the unitizer and do not change successful artifact identity. The registry also supplies the 8 MiB Source limit and the 397312-byte complete-unit JSON response bound consumed by both binaries. It owns no preparation encoder, object writer or durable settings/outcome revision.

MAX_PUBLICATION_RESPONSE_BYTES is 64 fixed-field evidence hits at 2048 serialized bytes each plus 4096 bytes
of generation/framing metadata: 135168 bytes. Both binaries consume this bound. Publication metadata carries
no unit text; U1's later framed text budget is a separate actual consumer.

## Concurrency contract

A SourceId and root namespace are generated from the OS random source and persisted. A path locator indexes a stored SourceId but does not determine it. A row update advances row_revision and checks both row_revision and registration_revision in the same redb write transaction. Registration IDs are not inferred from content or native file keys. Current S1 has no removal operation; a later removal/re-registration path must preserve or replace the identity guard before it is connected.
