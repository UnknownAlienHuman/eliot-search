# Ticket-issuance qualification

This directory contains two separate non-authoritative verification layers:

1. the read-only schema-v2 advisory planner;
2. the synthetic operation-level conformance corpus for the integration-owned issuance control plane.

Neither layer materializes context, issues a ticket or lease, authorizes implementation, publishes a package handoff, accepts a gate/wave, or advances launch state.

## Advisory planner v2

### Commands

```powershell
python -m py_compile `
  tools/plan-ticket-issuance.py `
  tools/ticket_issuance_planner_v2/core.py `
  tools/ticket_issuance_planner_v2/drafts.py `
  tools/ticket_issuance_planner_v2/context.py `
  tools/ticket_issuance_planner_v2/control.py `
  tools/ticket_issuance_planner_v2/plan.py `
  tools/validate-ticket-issuance-plan.py `
  qualification/ticket-issuance/fixture_plan_ticket_issuance_v2.py `
  qualification/ticket-issuance/test_plan_ticket_issuance_v2.py

python qualification/ticket-issuance/test_plan_ticket_issuance_v2.py
python tools/validate-ticket-issuance-plan.py --json
python tools/plan-ticket-issuance.py `
  --package search-contracts `
  --output artifacts/ticket-issuance-plans/search-contracts.json
```

The current zero-state repository run must produce:

```text
decision = BLOCKED_MISSING_SELECTION
mutations = []
authorizes_context_materialization = false
authorizes_ticket_issuance = false
creates_writer_lease = false
authorizes_implementation = false
publishes_package_handoff = false
advances_launch_state = false
```

That result is expected while no immutable base/writer/reviewer selection exists.

`cases-v2.toml` inventories 30 planner cases covering immutable Git-tree reads, selection validation,
schema-v2 fields, context ceilings, source and selector checks, prerequisite handoffs, control-record
conflicts, manual-only workflow policy, artifact-root fencing, deterministic JSON and zero authority.

## Operation conformance corpus

Files:

- [`manifest.toml`](manifest.toml) — exact paths, counts, authority nonclaims and evidence policy;
- [`baseline.toml`](baseline.toml) — locked operation, recovery, identity, context, lease and zero-state behavior;
- [`fixtures.toml`](fixtures.toml) — 52 synthetic fixture descriptors, never real records or actor assignments;
- [`probes.toml`](probes.toml) — 64 mandatory probes covering failures and recovery dispositions;
- [`TICKET_ISSUANCE_QUALIFICATION.md`](TICKET_ISSUANCE_QUALIFICATION.md) — execution and acceptance contract.

Machine operation ownership is defined by
[`../../swarm/control-plane-operations.toml`](../../swarm/control-plane-operations.toml). Record fields and
canonical layouts remain owned by the current schema-v2 control-plane schema/type registries.

Run:

```powershell
pwsh -NoProfile -File tools/validate-ticket-issuance-conformance.ps1
pwsh -NoProfile -File tools/validate-ticket-issuance-conformance.ps1 -Json
```

A structural PASS proves only corpus closure. Every probe remains `UNAVAILABLE` until immutable raw
output and an independent reviewer receipt exist.

## Evidence boundary

A green planner or conformance run is not:

- a materialized context;
- an issued assignment ticket;
- a writer lease or acknowledgement;
- a package submission, review or handoff;
- G0 evidence;
- a W0 receipt;
- permission to implement any package.
