# search-revision-crypto — consume only the accepted #598 handoff

This package is a narrow pure authenticated-encryption library, not a complete storage profile. It performs no
filesystem, redb, OS key-store, source, publication or recovery I/O and cannot itself establish durable key or
nonce authority.

## Revision-specific evidence

At original public-main baseline `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, the package contained a
19,316-byte AES-256-GCM v1 implementation and repository search found no public-main crate consumer. #598 also
named this area as private active overlap. Those size/caller findings are evidence at that revision, not a
permanent current-branch census.

Do not delete, integrate or implement a competing crypto/object path while #598 is active. After its accepted
public handoff, search exact current manifests, Rust/non-Rust callers, features, targets, tests and workflows.
Then choose one outcome in the same implementation cut:

1. retain a narrow pure primitive only if the accepted immutable-object owner directly uses its exact selected
   envelope, AAD and key/nonce contract; or
2. delete this package and workspace/dependency entries when the accepted owner implements the primitive
   elsewhere or selects plain storage.

## Boundary if retained

```text
selected key/nonce-use authority
+ exact bounded plaintext
+ complete expected AAD/profile identity
→ authenticated envelope

exact selected key
+ exact bounded envelope
+ complete expected AAD/profile identity
→ verified plaintext
```

The caller owns key acquisition, nonce/key-use continuity, object identity, durable publication and exact
reopen. The primitive returns no receipt and creates no provider, rotation, migration or recovery platform.

Do not preserve v1 bytes solely for pre-release compatibility. A read-only old-format path may remain only
while a real old root requires explicit access/export. New writes use one selected current envelope. Plain
storage uses no revision crypto.

Protect #598: do not inspect, reset or modify unpublished work, infer its final API, or create a second writer.
After code, run minimal Rust 1.98 check/Clippy for the retained primitive and actual object/key consumers, then
focused round-trip, wrong-key, wrong-AAD, corruption, length and no-secret-output cases.
