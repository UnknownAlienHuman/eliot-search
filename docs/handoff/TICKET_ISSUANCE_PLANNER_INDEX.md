# Ticket-issuance planner v2 index

## Authority and machine files

- [`TICKET_ISSUANCE_PLANNER_V2.md`](TICKET_ISSUANCE_PLANNER_V2.md) — read-only schema-v2 operation contract.
- [`TICKET_ISSUANCE_PLANNER_DIGEST_V2.md`](TICKET_ISSUANCE_PLANNER_DIGEST_V2.md) — non-circular digest rule.
- [`../../swarm/ticket-issuance-planner-v2.toml`](../../swarm/ticket-issuance-planner-v2.toml) — component registry.
- [`../../swarm/ticket-issuance-plan-schema-v2.toml`](../../swarm/ticket-issuance-plan-schema-v2.toml) — closed output and reason registry.
- [`../../swarm/ticket-issuance-plan-digest-v2.toml`](../../swarm/ticket-issuance-plan-digest-v2.toml) — digest profile.

## Executable tooling

- [`../../xtask/src/ticket_issuance_builder.rs`](../../xtask/src/ticket_issuance_builder.rs) — Rust advisory planner facade.
- [`../../xtask/src/ticket_issuance_builder/`](../../xtask/src/ticket_issuance_builder/) — immutable-tree selection, drafts, sources/selectors, handoffs, assembly and bounded output modules.
- [`../../tools/plan-ticket-issuance.ps1`](../../tools/plan-ticket-issuance.ps1) — locked Cargo compatibility entrypoint.
- [`../../xtask/src/ticket_issuance_validation.rs`](../../xtask/src/ticket_issuance_validation.rs) — registry/schema/digest/current-state structural validator.
- [`../../tools/validate-ticket-issuance-plan.ps1`](../../tools/validate-ticket-issuance-plan.ps1) — locked Cargo validator wrapper.

## Qualification

- [`../../qualification/ticket-issuance/cases-v2.toml`](../../qualification/ticket-issuance/cases-v2.toml) — 30-case inventory.
- [`../../xtask/tests/ticket_issuance_builder.rs`](../../xtask/tests/ticket_issuance_builder.rs) — deterministic zero-state, selection and output-boundary regression.
- [`../../xtask/tests/ticket_issuance_builder_runtime_boundary.rs`](../../xtask/tests/ticket_issuance_builder_runtime_boundary.rs) — Rust-only runtime and module-ownership guard.
- [`../../xtask/tests/ticket_issuance_validation.rs`](../../xtask/tests/ticket_issuance_validation.rs) — structural-validator regression.
- [`../../qualification/ticket-issuance/README.md`](../../qualification/ticket-issuance/README.md) — evidence boundary and exact commands.
- [`../../.github/workflows/ticket-issuance-plan.yml`](../../.github/workflows/ticket-issuance-plan.yml) — manual Windows qualification.

## Current disposition

```text
expected search-contracts decision: BLOCKED_MISSING_SELECTION
context materializer:              present as advisory tooling only
issued tickets:                    0
active leases:                     0
accepted package handoffs:         0
G0/W0:                             not accepted
```
