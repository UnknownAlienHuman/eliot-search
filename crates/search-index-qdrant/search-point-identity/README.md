# search-point-identity

**C14 — collision-safe point identity.**

**Status:** the active `main` package contains a substantive legacy point-identity implementation, but it
does not satisfy the current Architecture S11 canonical CBOR + full BLAKE3-256 profile. It must not be
used to mint the next production collection generation.

Draft PR #207 contains the intended canonical replacement and must be integrated atomically with the
S9.5 projection/publication/bridge stack.

## Current legacy source

The active source derives a bounded compact identity from immutable logical inputs and detects conflicts,
but it still uses the older key/profile and a package-local FNV-derived digest. That source is `LEGACY`,
not absent and not architecture-qualified.

## Required canonical contract

The accepted replacement owns:

- versioned `ProjectionPointKey` fields from Architecture S11.1;
- bounded deterministic canonical CBOR bytes;
- namespace-separated full BLAKE3-256 identity digest;
- an independently domain-separated 128-bit Qdrant UUID projection;
- exact comparison of UUID, full digest and independently stored identity fields;
- collision refusal before any upsert;
- deterministic golden bytes/digest/UUID fixtures.

## Must not own

- ad-hoc string or JSON identity derivation;
- source identity or source-membership policy;
- Qdrant transport or upsert execution;
- a mutable identity registry as product authority;
- claims that a truncated-ID collision is impossible.

## Integration rule

Do not combine the legacy `main` identity with the aligned S9.5 payload/manifests. Integrate #207 with
#200, #209 and #210, mint a new collection generation and rebuild. Existing experimental point IDs are
not silently adopted.

- **Product area:** Architecture S11
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Current status matrix:** [../../../docs/product/IMPLEMENTATION_STATUS.md](../../../docs/product/IMPLEMENTATION_STATUS.md)
