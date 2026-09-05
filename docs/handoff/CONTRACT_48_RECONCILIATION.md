# P00 issue #48 — exact closure proposed for independent review

Status: **PROPOSED_NOT_ACCEPTED**. Source: `8ef226d8dca4368e2fe83c37c870f56190b2c168`. Owner of shapes: `search-contracts`; contract-pack changes remain integration-owned. This file does not amend Part I, freeze a new digest, close #48 or authorize downstream consumers.

## Existing source and missing registry entries

| Name | Already present | Remaining closure |
|---|---|---|
| `UtcTimestamp` | `CANONICAL_TYPES.md` already requires RFC 3339 UTC `Z` and six fractional digits. `canonical.rs` implements fixed 27-byte parsing, calendar bounds and rejection of year zero/leap seconds. | Add the named type/visibility/codec/bounds entry; do not claim the timestamp format was wholly unspecified. |
| `MetadataKey` | `canonical.rs` defines a bounded nonempty key starting with an ASCII lowercase letter; remaining bytes allow lowercase/digit/underscore/dot/hyphen. | Register the key's exact length, canonical byte order and closed alphabet instead of permitting an ad-hoc string alias. |
| `UnresolvedSource` | `source.rs` defines `source_id` and bounded closed `reason_codes`; its constructor rejects an empty reason set. `SOURCE_GRAPH.md` already references the helper. | Register the full helper shape, nonempty reasons and visibility; check every codec/validation path, not only that constructor. |

`TYPE_REGISTRY.md` lacks these three named entries. `TYPE_COMPLETIONS.md` closes five other helpers, not these three. `CONTRACT_CHALLENGES.md` ends at PC-018 and does not accept #48.

## Proposed exact additions, preserving existing intended representation

- `UtcTimestamp`: scalar UTF-8 `YYYY-MM-DDTHH:MM:SS.ffffffZ`, exactly 27 ASCII bytes, valid proleptic Gregorian date, years 0001–9999, seconds 00–59. JSON and deterministic CBOR are text; alternate offsets/precision and leap seconds are rejected. Lexicographic order equals time order within this form.
- `MetadataKey`: 1–128 UTF-8 bytes restricted to ASCII `[a-z][a-z0-9_.-]*`; no case folding or Unicode normalization. Canonical bytewise ordering; duplicate metadata keys remain invalid. JSON/CBOR use the canonical text.
- `UnresolvedSource`: exactly `source_id: SourceId` and `reason_codes: bounded_set<SearchReasonCodeV1, MAX_REASON_CODES>` with at least one reason. It is a content-free server/shared-domain validation record, not automatically a permitted provider result. It adds no path, source text or authorization field.

## Review and conformance obligations

Add the named entries to the accepted P00 pack, account for them in `swarm/coverage/schemas-primitives.toml`, and update any affected canonical pack/schema digests only through the normal acceptance procedure. The registry's advertised symbol count must be recomputed, not incremented without enumerating aliases and types. No frozen old digest is silently rewritten.

Execute positive/negative JSON and CBOR cases, timestamp calendar/order/precision cases, metadata 0/1/128/129-byte/alphabet/duplicate cases, and empty/unknown/duplicate/over-limit reason cases. Preserve the existing tests instead of assigning a second implementation.

**Additional review point:** `UnresolvedSource` exposes public fields, so `new()` is not the only construction path. `SourceOwnerCutoverReceipt::validate()` currently checks owner states and cutover timestamp ordering, not each nested reason set. Check canonical decoders and serialization/validation entrypoints for empty-reason bypass before calling this invariant enforced. This is a source-level validation gap to test, not proof of a reachable authorization exploit in the primary daemon. Do not conflate structural helper validity with the separate rule that successful owner cutover has zero unresolved sources.

The exact inspected blobs are: `TYPE_REGISTRY.md` = `bb56b8c2a95946b3e7bbd274ee12ea68fff3d709`; `TYPE_COMPLETIONS.md` = `1dfe6453fdacadf555e03f75e596da0d0d3b3a63`; `CANONICAL_TYPES.md` = `6bfc7519f7cfb40f47b54a063c4e2c82b1b956f7`; `CONTRACT_CHALLENGES.md` = `a382d4c2fef84587a2153941d2ffa306b8a00900`; `canonical.rs` = `f9447662e5e55cdb8ec8a79f01f9d815c3b98dbd`; `source.rs` = `6794bd433478ff1b426b44cd9a03566f0037361b`.

No Rust or native tests were executed for this review. Independent acceptance is still required.
