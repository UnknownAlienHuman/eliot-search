# Context artifact candidate builder v1 index

## Contracts and machine files

- [`CONTEXT_ARTIFACT_CANDIDATE_V1.md`](CONTEXT_ARTIFACT_CANDIDATE_V1.md) — exact non-authoritative build contract and bundle framing.
- [`CONTEXT_ARTIFACT_CANDIDATE_DIGEST_V1.md`](CONTEXT_ARTIFACT_CANDIDATE_DIGEST_V1.md) — artifact, candidate and metadata digest separation.
- [`../../swarm/context-artifact-builder-v1.toml`](../../swarm/context-artifact-builder-v1.toml) — component registry and authority boundary.
- [`../../swarm/context-artifact-candidate-schema-v1.toml`](../../swarm/context-artifact-candidate-schema-v1.toml) — closed candidate shape and unresolved manifest fields.
- [`../../swarm/context-artifact-candidate-digest-v1.toml`](../../swarm/context-artifact-candidate-digest-v1.toml) — machine digest profile.

## Executable tooling

- [`../../tools/build-context-artifact-candidate.py`](../../tools/build-context-artifact-candidate.py) — Python CLI.
- [`../../tools/context_artifact_builder_v1/`](../../tools/context_artifact_builder_v1/) — bounded core, bundle, extraction and build modules.
- [`../../tools/build-context-artifact-candidate.ps1`](../../tools/build-context-artifact-candidate.ps1) — Windows wrapper.
- [`../../tools/validate-context-artifact-candidate.py`](../../tools/validate-context-artifact-candidate.py) — structural/current-tree validator.
- [`../../tools/validate-context-artifact-candidate.ps1`](../../tools/validate-context-artifact-candidate.ps1) — Windows validator wrapper.

## Qualification

- [`../../qualification/context-artifact/cases-v1.toml`](../../qualification/context-artifact/cases-v1.toml) — twenty-case inventory.
- [`../../qualification/context-artifact/test_context_artifact_candidate_v1.py`](../../qualification/context-artifact/test_context_artifact_candidate_v1.py) — committed-Git conformance suite.
- [`../../qualification/context-artifact/README.md`](../../qualification/context-artifact/README.md) — commands and evidence ceiling.
- [`../../.github/workflows/context-artifact-candidate.yml`](../../.github/workflows/context-artifact-candidate.yml) — manual Windows qualification.

## Current disposition

```text
search-contracts artifact candidate: buildable at an exact commit
immutable artifact store/ref:         unselected / absent
materializer and reviewer:            unselected
committed context_manifest_v1:        absent
issued tickets:                       0
active leases:                        0
implementation authority:             absent
```
