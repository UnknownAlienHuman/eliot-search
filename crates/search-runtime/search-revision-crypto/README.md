# search-revision-crypto — consume only the accepted #598 handoff

Public `main` contains one 19,316-byte AES-256-GCM v1 library with a random 96-bit nonce, SHA-256-bearing AAD,
versioned envelope and zeroized key/plaintext wrappers. Repository source search finds no public-main crate
consumer.

This package is nevertheless an explicit private owner inside active #598. Do not delete, integrate or build
a competing crypto/storage path while that work is active. Consume only its accepted public handoff.

## Current public-main mismatch

The current library is not the selected end-to-end storage profile by itself:

- it owns only pure seal/open bytes;
- it has no durable nonce/key-use accounting;
- it loads/persists no selected key;
- it writes/reads no immutable object;
- no normal source-retention caller invokes it;
- its random-nonce v1 envelope differs from the selected-reference/current-format work described by #598.

A good standalone AEAD primitive is not evidence that encrypted retention works. Do not connect this v1
library to a second object store or wrap it in generic key/provider/receipt machinery.

## Handoff disposition

After #598 publishes an accepted result, exactly one of these is valid:

1. this package becomes the narrow pure primitive actually used by the accepted immutable-object owner, with
   the exact selected envelope/AAD/nonce/use contract and direct callers; or
2. the accepted implementation owns that primitive elsewhere and this package/workspace member is deleted.

Plain storage uses no revision crypto. The encrypted profile may depend only on the selected concrete Windows
key-use owner and the immutable object writer; no universal secret catalogue, rotation/migration platform or
cryptography on unrelated state transitions.

## Required narrow boundary if retained

```text
selected one-use key/nonce permission
+ exact plaintext
+ exact complete AAD/profile identity
→ bounded authenticated envelope

exact selected key
+ exact envelope
+ exact expected AAD/profile identity
→ verified plaintext
```

The pure primitive performs no filesystem, redb, Credential Manager/DPAPI, policy, source, publication or
recovery I/O. It cannot issue key-use authority or claim durability. Callers own finite lengths, key/nonce
continuity, object publication and exact reopen.

Do not preserve v1 bytes merely for pre-release compatibility. A read-only legacy path may remain only when
a real old root must be explicitly opened; new current-format writes use one selected envelope.

#598 remains active private work. Do not inspect/reset/rebase its branch, modify its unpublished API or assign
a competing writer. This public audit records only current-main reachability and the allowed handoff outcome.
