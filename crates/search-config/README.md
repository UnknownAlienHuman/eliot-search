# search-config

**Deterministic, I/O-free typed configuration and reload planning.**

This package owns finite typed documents, registry/layer semantics, section equality identities,
effective snapshot fingerprints, redaction, deltas and plans. Capability packages own section
meaning and runtime application. Configuration equality never proves validation or applied effects.

New snapshots compute BLAKE3 over shared deterministic CBOR under
`eliot/cbor/config-snapshot/v2`. The config owner computes each section digest from exact typed
fields, requested/selected profiles and supplied source/reset provenance; callers supply no digest.
Private snapshot compute consumes its existing ordered bounded map, without another sort, full
preimage or public arbitrary iterator.

This is VERSIONED_REBUILD, independently of the current schema-1 TOML parser. SHA-v1 bytes can be
restored only with `ConfigFingerprint::restore_legacy_v1` for migration/comparison. They cannot
construct a current effective snapshot. Equality preserves algorithm/profile/revision even if raw
bytes coincide. The immediate daemon status emits the full tag under its explicit v2 schema and
rejects report/snapshot identity mismatch. Runtime application/readback remains #333.

Borrowed identifier, value and list extents plus fixed scalar/digest payload are checked before
temporary canonical rows. Assembly also charges names/owners before map clones and insertion.
One shared encoder decides exact domain-inclusive CBOR bytes under the 8 MiB hard ceiling.
Logical payload budgets are not measured allocator residency or serialized JSON/Debug size.

File/env/CLI documents retain the shared builder, descriptor/secret/precedence rules and admission
ceilings: W1 identifiers 1 MiB, values 4 MiB, total list items 8,192. Captured document Debug emits
fixed primitive metadata. Redacted views charge complete tagged header and entry payload before
copies/hashing and omit over-budget entries whole. Bounded redacted path digests retain exact
standard SHA-256 through the shared owner; no production legacy fingerprint encoder remains.

Identity/compatibility/diagnostic fixtures are AUTHORED_NOT_RUN under #352. The maintained TOML
parser cutover remains #389/#238; integrated/native/installed/scale/release qualification remains #215.
No donor, manifest or lockfile changes belong to this identity slice.

## Owns

- config source/layer identity and deterministic merge semantics
- top-level section registry and unknown-section rejection
- fixed/security-floor/override classification
- effective snapshot fingerprint and redacted diagnostic projection
- change classification: live, security barrier, restart, new generation or reject

## Must not own

- reading files/environment/process arguments
- package-specific runtime state or vendor configuration objects
- plaintext secrets or permissive fallback on unknown settings
- silently applying restart/rebuild/security changes as live updates

- **Delivery wave:** W1 / P01
- **Soft source-line target:** 5,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
