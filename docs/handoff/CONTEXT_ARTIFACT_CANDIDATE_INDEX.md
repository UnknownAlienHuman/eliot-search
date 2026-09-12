# Context artifact candidate builder v1 index

## Contracts and machine files

- [`CONTEXT_ARTIFACT_CANDIDATE_V1.md`](CONTEXT_ARTIFACT_CANDIDATE_V1.md) — exact non-authoritative build contract and bundle framing.
- [`CONTEXT_ARTIFACT_CANDIDATE_DIGEST_V1.md`](CONTEXT_ARTIFACT_CANDIDATE_DIGEST_V1.md) — artifact, candidate and metadata digest separation.
- [`../../swarm/context-artifact-builder-v1.toml`](../../swarm/context-artifact-builder-v1.toml) — component registry and authority boundary.
- [`../../swarm/context-artifact-candidate-schema-v1.toml`](../../swarm/context-artifact-candidate-schema-v1.toml) — closed candidate shape and unresolved manifest fields.
- [`../../swarm/context-artifact-candidate-digest-v1.toml`](../../swarm/context-artifact-candidate-digest-v1.toml) — machine digest profile.

## Executable tooling

- [`../../xtask/src/context_artifact_builder.rs`](../../xtask/src/context_artifact_builder.rs) — Rust builder facade.
- [`../../xtask/src/context_artifact_builder/`](../../xtask/src/context_artifact_builder/) — immutable-tree preflight, extraction, assembly and exact local publication.
- [`../../xtask/src/context_artifact.rs`](../../xtask/src/context_artifact.rs) — canonical candidate/bundle primitives.
- [`../../xtask/src/context_artifact_io.rs`](../../xtask/src/context_artifact_io.rs) — output-root fencing and idempotent write/readback.
- [`../../tools/build-context-artifact-candidate.ps1`](../../tools/build-context-artifact-candidate.ps1) — Windows wrapper for the locked Cargo command.
- [`../../xtask/src/context_artifact_validation.rs`](../../xtask/src/context_artifact_validation.rs) — registry/schema/digest/authority structural validator.
- [`../../tools/validate-context-artifact-candidate.ps1`](../../tools/validate-context-artifact-candidate.ps1) — locked Cargo validator wrapper.

## Qualification

- [`../../qualification/context-artifact/cases-v1.toml`](../../qualification/context-artifact/cases-v1.toml) — twenty-case inventory.
- [`../../xtask/tests/context_artifact_builder.rs`](../../xtask/tests/context_artifact_builder.rs) — exact-commit current candidate regression.
- [`../../xtask/tests/context_artifact_parity.rs`](../../xtask/tests/context_artifact_parity.rs) — captured compatibility vectors executed entirely in Rust.
- [`../../xtask/tests/context_artifact_rust_io.rs`](../../xtask/tests/context_artifact_rust_io.rs) — output-root and idempotent publication regression.
- [`../../xtask/tests/context_artifact_validation.rs`](../../xtask/tests/context_artifact_validation.rs) — structural-validator regression.
- [`../../qualification/context-artifact/README.md`](../../qualification/context-artifact/README.md) — commands and evidence ceiling.
- [`../../.github/workflows/context-artifact-candidate.yml`](../../.github/workflows/context-artifact-candidate.yml) — manual Windows qualification.

## Next materialization prerequisites

- [`ACCEPTED_EVIDENCE_DIGEST_V1.md`](ACCEPTED_EVIDENCE_DIGEST_V1.md) — exact semantic digest derived from an accepted handoff's committed `evidence[]` array.
- [`../../swarm/type-rule-profiles-v1.toml`](../../swarm/type-rule-profiles-v1.toml) — binds `OrderedAcceptedPackageHandoff.evidence_digest` to that profile.
- [`../../swarm/accepted-evidence-digest-v1.toml`](../../swarm/accepted-evidence-digest-v1.toml) — canonical machine format and limits.
- [`CONTEXT_MATERIALIZATION_PLAN_INDEX.md`](CONTEXT_MATERIALIZATION_PLAN_INDEX.md) — compiles the candidate, immutable artifact/readback declaration, actor selection and dual signature refs into exact prospective `context_manifest_v1` bytes while preserving zero authority.

The immutable handoff reference remains authority. The derived evidence digest is required only to build
the bounded accepted-handoff projection for a future `context_manifest_v1`; it cannot replace exact
handoff readback or create package, gate, wave or implementation authority.

The materialization planner is also non-authoritative: it writes only ignored plan/payload/prospective
manifest artifacts, never mutates `swarm/context-manifests/**`, and leaves every authority field false.

## Current disposition

```text
search-contracts artifact candidate: buildable at an exact commit through Rust
accepted-handoff evidence profile:    specified; no accepted handoff currently exists
materialization plan compiler:         available; external store/actor/signature inputs absent
immutable artifact store/ref:         unselected / absent
materializer and reviewer:            unselected
committed context_manifest_v1:        absent
issued tickets:                       0
active leases:                        0
implementation authority:             absent
```
