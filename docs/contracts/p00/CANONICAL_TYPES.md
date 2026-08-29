# Canonical primitive and encoding rules

## Precedence correction

`SourceOwnerGeneration` is `Blake3Digest32`, as required by Architecture S7.2.1. The
`NonZeroU64` sketch in H3.1 is a subordinate handoff typo. `OwnerEpoch` remains `NonZeroU64`.

## Primitive wrappers

| Type family | Canonical rule |
|---|---|
| UUID-backed IDs | distinct newtype; JSON lower-case hyphenated UUID; deterministic CBOR 16-byte value |
| `OwnerEpoch` | non-zero unsigned 64-bit integer |
| `Epoch` | signed 64-bit integer, `0 <= value < i64::MAX` |
| revision/generation counters | dedicated unsigned newtype; zero only when schema defines initial state |
| `Blake3Digest32` / `Sha256Digest32` | 32 bytes; JSON 64 lower-case hex characters |
| `SourceOwnerGeneration` | dedicated BLAKE3 digest wrapper, never integer |
| opaque identity/reference | non-empty bounded UTF-8 or canonical bytes; no consumer parsing |
| raw bytes in JSON | base64url without padding; never implicit UTF-8 |
| `OpaqueHandleToken` | 32–64 CSPRNG bytes; base64url; never derived from source/plan/path/identity |
| `HandleTokenDigest` | domain-separated BLAKE3-256 of exact token; server-side only |
| timestamp | RFC 3339 UTC `Z`, exactly six fractional digits at load-bearing boundaries |
| duration/deadline | unsigned milliseconds, never floating point |

P00 publishes explicit bounds for every opaque/list/string/byte field. Default outer frame limit is
8 MiB unless a narrower schema applies.

Opaque tokens are bearer locators, not authorization decisions. They are redacted from logs,
telemetry, panic/debug output and canonical identity inputs. `HandleId` is non-secret correlation
identity; valid token plus current authorization is required.

## Versioned content digest

```yaml
VersionedContentDigest:
  algorithm: blake3_256 | sha256
  bytes: fixed_32_bytes
```

Native Search objects use BLAKE3. Protocol SHA-256 is computed from verified bytes, never relabeled.

## Canonical serialization

- Provider transport: `u32` little-endian length + UTF-8 JSON.
- Identity/fingerprint inputs: RFC 8949 deterministic CBOR over versioned structs.
- Enum discriminants/map keys are fixed; map iteration order is not observable.
- Floating point is forbidden in identity/fingerprint inputs.
- PDF coordinates are result data unless a future fixed-point identity mapping is defined.
- JSON reserialization is never an identity input.
- Opaque handle tokens are excluded from identity/fingerprint canonicalization.

## Domain separation

```text
eliot-search/source-owner-generation/v1
eliot-search/object-residency-key/v1
eliot-search/point-identity/v1
eliot-search/query-snapshot-fingerprint/v1
eliot-search/plan-fingerprint/v1
eliot-search/schema-identity/v1
eliot-search/receipt/v1
eliot-search/handle-token-digest/v1
eliot-search/continuation-token-digest/v1
```

Prefix, schema version and canonical payload are hashed. Golden bytes/digests are required.

## Unknown fields

Unknown load-bearing security, scope, identity, budget, currentness, lifecycle and ownership fields fail
closed. Minor extensions require negotiation and explicit non-load-bearing registration; generic
`deny_unknown_fields` alone is insufficient.
