# Immutable plain-object store

## Current API

| Symbol | Behavior |
|---|---|
| store_source_bytes | Consumes the one acquired Vec, computes a maintained BLAKE3 digest and exact length, publishes a version-1 source object without clobbering, then invokes read_source_object before returning ObjectRef. |
| read_source_object | Reads only the addressed retained object, checks its actual length and BLAKE3 digest, and returns the exact verified retained bytes. |
| ObjectRef | Carries format version, source kind, digest, and byte length. It carries no key, cipher, receipt, residency, or security profile. |

Object names are derived only from version, kind, and the verified content digest under PlainRootGuard.object_dir. A locator is never reopened as a substitute. Conflicting existing bytes fail. A failed source-row CAS can leave an unreferenced immutable object; it cannot overwrite the current head.

## Actual caller

The daemon keeps the root guard for the operation and performs object I/O before the short control transaction. read_source_object is the actual post-publication readback consumer used by store_source_bytes. No writer transaction spans file I/O.

## Bounds and errors

Source objects are capped at 8 MiB. Missing objects, wrong length, corruption, conflicting publication, deadline, cancellation, and I/O failure are distinct content-free outcomes. A zero-byte source is represented by the actual BLAKE3 hash and zero length.
