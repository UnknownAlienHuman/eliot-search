# DIRECT SHA-256 API repair — first multipart framing definition

PR #142 identifies a real integration defect at `b80483b`: fifteen daemon modules
call four functions absent from `src/sha256.rs`. This decision defines that missing
local API; it does not replace a previously implemented multipart encoding.

## Exact byte contract

- `digest(&[u8]) -> [u8; 32]` is ordinary SHA-256 of the bytes, using the existing
  `digest_bytes` implementation. The old `Sha256Digest` API is retained.
- `hex(&[u8]) -> String` is lower-case hex of arbitrary bytes, not just digests.
  This matters for existing bounded revision-range responses.
- `decode_digest(&str) -> Option<[u8; 32]>` requires exactly 64 ASCII hexadecimal
  characters, accepts the existing upper/lower-case parser, and never trims input.
- `digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32]` hashes this exact frame:

```text
ASCII("eliot-search/sha256-parts/v1") || 0x00
|| u64_be(domain byte length) || domain bytes
|| u64_be(part count)
|| for each part: u64_be(part byte length) || part bytes
```

No text normalization, implicit separators, host-size integer encoding, reordered
parts or skipped empty parts. The concrete slice type admits mixed fixed arrays
and vectors at existing call sites without truncation or caller casts. Each caller
still applies its finite input budget. The helper builds one framed preimage;
this is not a new streaming or hard-memory-budget implementation.

Independent SHA-256 fixture for domain `test/domain/v1` and parts `abc`, empty,
`def`: `ab1b291fe982137e0741dc711096200882e40d9cd32fd5a07b6177d7493b9078`.
Tests also retain standard raw vectors, arbitrary-byte hex, invalid decode input,
framing separation and the previously failing mixed-array call shapes.

## Compatibility and trust limit

The baseline has no implementation or golden fixture for `digest_parts`; its
historical byte format cannot be recovered from call sites. This framing is an
explicit first implementation, NOT a claim that externally produced DIRECT roots
are compatible. Do not migrate, re-key or mutate an externally populated root
without identifying its producer and verifying exact stored identities and format.
Use a disposable fresh root for the initial primary-binary validation. No migration,
external-root qualification or production activation is performed by this repair.
Existing raw SHA-256/newtype operations and redb codecs are unchanged. These bytes
must not be cast to a BLAKE3 digest or treated as keyed authentication.

This is a build/integration repair, not approval of the development catalog as the
canonical source owner. T10/T11 still replace that catalog through verified migration.
The five new Rust tests and complete primary-binary build require execution; the
local authoring environment has no Cargo (attempted check exited 127).
