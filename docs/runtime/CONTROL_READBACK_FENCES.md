# Protected readback and control snapshot publication

## Protected revision decoding

`RevisionProtector::unprotect` accepts only the protected envelope. Raw bytes are refused even when
their SHA-256 and length match the requested revision. This applies on every platform; a valid protected
envelope on a platform without DPAPI returns `DIRECT_REVISION_ENCRYPTION_UNAVAILABLE`, never its body
as plaintext. Explicit development `.bin` reads and referenced-plaintext migration remain separate.

The existing v1 byte layout, key derivation and IDs are unchanged. Outer and authenticated inner
bindings are validated against the expected namespace, key, revision, content digest and byte length.
Truncated headers, zero ciphertext, trailing bytes and oversized declared plaintext fail closed.
Protection also checks the supplied plaintext digest before calling native encryption.

Root-secret and inner-envelope allocations use the existing `zeroize` dependency. Inner plaintext is
validated while borrowed from a zeroizing owner; rejected content is not copied into an unguarded
return allocation. Successful returned bytes retain the caller's existing lifetime responsibilities.
The format marker alone is not evidence of authentication, source authority or currentness.

## Control publication

The public `search_control_redb::ControlSnapshotPublisher` guards the existing immutable snapshot
pointer. The reference transition model stays private behind that boundary. Both real redb publication
methods resolve the guarded public type; they continue to verify committed disk state first.

A published generation or owner epoch cannot decrease. Installation, root, path and schema bindings
cannot change on an existing publisher. A verified owner successor may advance the epoch while keeping
the same data generation, but equal-generation records must be identical. A known operation ID cannot
be substituted at the same generation. Empty/duplicate changed-key receipts, nonconsecutive transaction
generations, and malformed record ordering are rejected before pointer replacement.

A failed publication preserves the exact existing `Arc`. Recovery of an older empty model journal
cannot erase a populated publisher. A new installation requires a new publisher; resetting a publisher
is not a supported shortcut for admitting an unverified replacement database.

These checks establish local publication ordering, not the authenticity of arbitrary caller-supplied
receipts. Exact storage readback and live owner authorization remain mandatory. There is no new search
index, database owner, Python service, dependency version or persistence format.

## Verification boundary

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test -p eliot-searchd --bin eliot-searchd --locked
cargo +1.98.0 test -p search-control-redb --lib --locked
```

Twenty regression cases were added: twelve envelope/protection cases and eight publication cases.
Three protection cases use real Windows DPAPI without creating persistent test credentials. Two
publication cases use real redb databases, covering foreign-root rejection and owner handoff.
Synthetic envelope fixtures test parsing only, never cryptographic qualification.

These Rust commands and tests were not executed in the authoring environment. Local checks covered
lexical tokenization, the unchanged legacy field layout and exact Git blob identity, not compilation.
Primary-daemon redb migration, durable preparation manifests and live Qdrant integration remain
unfinished; these fixes do not advance their acceptance gates.
