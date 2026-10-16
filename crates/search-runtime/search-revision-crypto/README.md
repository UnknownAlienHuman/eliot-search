# search-revision-crypto

Authenticated encryption boundary for immutable source-revision bytes.

The legacy version-one profile uses AES-256-GCM with a fresh operating-system random 96-bit nonce.
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

The separate `selected_v2` primitive uses the `ELSREV02` frame and complete canonical
Current, pre-seal occurrence and six-domain residency records in AAD, together with
the logical operation, reserved counter, deterministic nonce, raw BLAKE3 digest and
plaintext length. Its exact AAD length and authenticated-block charge are exposed
for the durable key-use owner. Version-one bytes are never relabelled as version two.

The new primitive supplies equality and AEAD only. It performs no source admission,
key selection, nonce reservation, access check, clock or lease issuance. Product use
requires the closed Windows consumer, an actual fresh C02 reservation and current
source/policy/readback checks. Those consumer adapters are still being implemented
under #598; this source does not enable product retention or serving by itself.
