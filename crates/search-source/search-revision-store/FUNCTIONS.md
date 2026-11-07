# Immutable plain-object store

## Current API

| Symbol | Behavior |
|---|---|
| store_object | Borrows bytes with an explicit ObjectKind and maximum_bytes, computes maintained BLAKE3 and exact length, publishes without clobbering, then invokes read_object before returning ObjectRef. |
| read_object | Reads only the addressed retained object under the explicit allowance, checks its actual length and BLAKE3, and returns the exact verified bytes. |
| read_object_in | Shared read-only verification from an already validated plain object directory, including an archive copy; retains regular/no-reparse/size/digest/budget failures without constructing/adopting a runnable root. |
| ObjectRef | Carries format version, Source or Prepared kind, digest and byte length. It carries no key, cipher, receipt, residency or security profile. |

Object names are v1-source-DIGEST or v1-prepared-DIGEST under PlainRootGuard.object_dir. Source names/layout remain unchanged. A locator is never reopened as a substitute. Conflicting existing bytes fail. A failed source/preparation CAS can leave an unreferenced immutable object; it cannot overwrite the current head.

## Actual caller

The daemon retains the root guard. Retain-file passes its acquired buffer as a slice; prepare-source consumes a verified retained source Vec and passes its complete encoded artifact to this same writer. Object readback and saved-artifact decoding precede the short control transaction. No writer transaction spans object I/O.

Archive export uses read_object_in to verify copied objects independently of source bytes and ordinary root-open
semantics. Header/closure validation belongs to the archive/CONTROL consumer; this reader creates no import API.

## Bounds and errors

Source objects remain capped at 8 MiB even if a caller supplies a larger allowance. Prepared objects require an explicit encoded-byte allowance; the current daemon supplies 64 MiB. Missing objects, wrong length, corruption, conflicting publication, deadline, cancellation and I/O failure are distinct content-free outcomes. A zero-byte source has its actual BLAKE3 hash and zero length; an empty Prepared artifact still contains its real binding/header.
