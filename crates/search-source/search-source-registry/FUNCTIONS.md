# Source-row values

## Current types

| Type | Responsibility |
|---|---|
| CurrentSourceRecord | Persisted current source identity, locator, native file key, ObjectRef, row revision, and registration revision. |
| ObjectRef | Format version, source kind, BLAKE3 digest, and exact byte length. |
| NativeFileKey | Exact Unix device/inode or Windows volume/file-index pair. Unsupported native IDs are not replaced with path or digest guesses. |

search-control-redb owns table definitions, random ID creation, locator lookup, row encoding, and the atomic CAS. This crate supplies no registry, journal, compatibility codec, admission receipt, history, or replay API.

## Concurrency contract

A SourceId and root namespace are generated from the OS random source and persisted. A path locator indexes a stored SourceId but does not determine it. A row update advances row_revision and checks both row_revision and registration_revision in the same redb write transaction. Registration IDs are not inferred from content or native file keys. Current S1 has no removal operation; a later removal/re-registration path must preserve or replace the identity guard before it is connected.
