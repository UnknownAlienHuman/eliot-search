# Ticket-issuance planner v2 index

## Authority and machine files

- [`TICKET_ISSUANCE_PLANNER_V2.md`](TICKET_ISSUANCE_PLANNER_V2.md) — read-only schema-v2 operation contract.
- [`TICKET_ISSUANCE_PLANNER_DIGEST_V2.md`](TICKET_ISSUANCE_PLANNER_DIGEST_V2.md) — non-circular digest rule.
- [`../../swarm/ticket-issuance-planner-v2.toml`](../../swarm/ticket-issuance-planner-v2.toml) — component registry.
- [`../../swarm/ticket-issuance-plan-schema-v2.toml`](../../swarm/ticket-issuance-plan-schema-v2.toml) — closed output and reason registry.
- [`../../swarm/ticket-issuance-plan-digest-v2.toml`](../../swarm/ticket-issuance-plan-digest-v2.toml) — digest profile.

## Executable tooling

- [`../../tools/plan-ticket-issuance.py`](../../tools/plan-ticket-issuance.py) — dependency-free CLI and compatibility entrypoint.
- [`../../tools/ticket_issuance_planner_v2/`](../../tools/ticket_issuance_planner_v2/) — bounded immutable-tree, draft, context, control and plan modules.
- [`../../tools/plan-ticket-issuance.ps1`](../../tools/plan-ticket-issuance.ps1) — Windows wrapper.
- [`../../tools/validate-ticket-issuance-plan.py`](../../tools/validate-ticket-issuance-plan.py) — registry, corpus and current-tree validator.
- [`../../tools/validate-ticket-issuance-plan.ps1`](../../tools/validate-ticket-issuance-plan.ps1) — Windows validator wrapper.

## Qualification

- [`../../qualification/ticket-issuance/cases-v2.toml`](../../qualification/ticket-issuance/cases-v2.toml) — 30-case inventory.
- [`../../qualification/ticket-issuance/fixture_plan_ticket_issuance_v2.py`](../../qualification/ticket-issuance/fixture_plan_ticket_issuance_v2.py) — deterministic committed-Git fixture.
- [`../../qualification/ticket-issuance/test_plan_ticket_issuance_v2.py`](../../qualification/ticket-issuance/test_plan_ticket_issuance_v2.py) — substantive conformance suite.
- [`../../qualification/ticket-issuance/README.md`](../../qualification/ticket-issuance/README.md) — evidence boundary.
- [`../../.github/workflows/ticket-issuance-plan.yml`](../../.github/workflows/ticket-issuance-plan.yml) — manual Windows qualification.

## Current disposition

```text
expected search-contracts decision: BLOCKED_MISSING_SELECTION
context materializer:              absent
issued tickets:                    0
active leases:                     0
accepted package handoffs:         0
G0/W0:                             not accepted
```
