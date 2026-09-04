# search-revision-crypto

Authenticated encryption boundary for immutable source-revision bytes.

The package uses AES-256-GCM with a fresh operating-system random 96-bit nonce.
Associated data binds every ciphertext to:

- monotone data-encryption-key generation;
- stable source plus immutable revision identity digest;
- complete object-residency key digest;
- encryption/profile digest;
- plaintext SHA-256 and exact byte length.

The binary envelope is versioned, bounded, strictly decoded, and rejects trailing,
truncated, unknown-version, unknown-algorithm, cross-revision, cross-residency,
and modified ciphertext.

`RevisionKey` and decrypted `PlaintextRevision` buffers are redacted and zeroized
on drop. The key has no persistence method. On Windows its exact 32 bytes are to
be wrapped by `search-os-secrets-windows`; only the DPAPI-protected key and the
AES-GCM revision envelope may be stored durably.
