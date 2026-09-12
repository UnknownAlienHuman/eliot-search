# Context materialization plan index

## Machine contracts

- [`../../swarm/context-materialization-planner-v1.toml`](../../swarm/context-materialization-planner-v1.toml)
- [`../../swarm/context-materialization-plan-schema-v1.toml`](../../swarm/context-materialization-plan-schema-v1.toml)
- [`../../swarm/context-materialization-plan-digest-v1.toml`](../../swarm/context-materialization-plan-digest-v1.toml)
- [`../../swarm/context-manifest-instance-v1.toml`](../../swarm/context-manifest-instance-v1.toml)
- [`../../swarm/context-manifest-renderer-v1.toml`](../../swarm/context-manifest-renderer-v1.toml)
- [`ACCEPTED_EVIDENCE_DIGEST_V1.md`](ACCEPTED_EVIDENCE_DIGEST_V1.md)

## Rust tooling

- [`CONTEXT_MATERIALIZATION_PLAN_V1.md`](CONTEXT_MATERIALIZATION_PLAN_V1.md)
- [`../../xtask/src/context_materialization.rs`](../../xtask/src/context_materialization.rs) — closed primitives and digest/reference grammar.
- [`../../xtask/src/context_materialization_builder.rs`](../../xtask/src/context_materialization_builder.rs) — executable planner facade.
- [`../../xtask/src/context_materialization_builder/`](../../xtask/src/context_materialization_builder/) — candidate/selection loading, handoff projection, prospective TOML rendering, plan assembly and ordinary output publication.
- [`../../tools/plan-context-materialization.ps1`](../../tools/plan-context-materialization.ps1) — locked Cargo compatibility wrapper.
- [`../../xtask/tests/context_materialization_builder.rs`](../../xtask/tests/context_materialization_builder.rs) — behavioral corpus.
- [`../../qualification/context-materialization/`](../../qualification/context-materialization/) — case inventory, commands and evidence ceiling.

## Authority ceiling

```text
prospective payload/manifest: ordinary ignored artifacts
context_manifest_v1 instance: absent
control record mutations:    empty
implementation authority:    false
``` 
