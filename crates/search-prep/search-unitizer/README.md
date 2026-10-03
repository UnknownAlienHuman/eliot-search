# search-unitizer

**C09 — Deterministic unitization.**

**Status:** deterministic exact-range unitization and durable profile-bound manifest behavior are implemented; Rust execution and qualification evidence remain pending.

Turn a materialization into deterministic unit occurrences and an immutable unit manifest.

## Owns

Durable manifests use `exact-unit-manifest/v2` with BLAKE3-framed profile,
unit and manifest identities. The decoder quarantines v1 manifests because
their stored `Blake3_256` label was backed by a non-cryptographic digest; they
must be regenerated from exact retained inputs before reunitization and
reprojection.

- unitizer profiles
- `UnitOccurrence` creation
- native anchor preservation
- ordinal/structural identity rules
- unit manifest digest and determinism

## Must not own

- ranking
- assuming unit stability across arbitrary reparses
- compiler certainty
- Qdrant point transport
- opening source stores directly instead of consuming immutable contract inputs

- **Delivery wave:** W2 / P04-P06
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
