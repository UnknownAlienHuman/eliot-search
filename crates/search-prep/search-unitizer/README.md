# search-unitizer

**C09 — Deterministic unitization.**

**Status:** deterministic exact-range unitization and durable profile-bound manifest behavior are implemented; Rust execution and qualification evidence remain pending.

Turn a materialization into deterministic unit occurrences and an immutable unit manifest.

## Owns

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
