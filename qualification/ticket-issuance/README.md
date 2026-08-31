# Ticket-issuance planner v2 qualification

This directory qualifies only the read-only schema-v2 advisory planner.

## Commands

```powershell
python -m py_compile `
  tools/plan-ticket-issuance.py `
  tools/validate-ticket-issuance-plan.py `
  qualification/ticket-issuance/test_plan_ticket_issuance_v2.py

python qualification/ticket-issuance/test_plan_ticket_issuance_v2.py
python tools/validate-ticket-issuance-plan.py --json
python tools/plan-ticket-issuance.py `
  --package search-contracts `
  --output artifacts/ticket-issuance-plans/search-contracts.json
```

The current repository run must produce:

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

That result is expected because no immutable base/writer/reviewer selection exists.

## Corpus

`cases-v2.toml` inventories 30 cases covering:

- immutable Git-tree reads and working-tree exclusion;
- complete, partial and invalid selections;
- schema-v2 ticket/context fields and rejection of schema-v1 aliases;
- exact contracts-pack and ordinary context ceilings;
- source blob, UTF-8, forbidden path and selector checks;
- conditional accepted handoffs and signed-payload readback;
- current-package records, nested metadata bypass and W0 conflicts;
- manual-only workflow policy;
- artifact-root output fencing;
- deterministic canonical JSON, non-circular digest and zero authority.

## Evidence boundary

A green run is not:

- materialized context;
- an issued assignment ticket;
- a writer lease or acknowledgement;
- a package submission, review or handoff;
- G0 evidence;
- a W0 receipt;
- permission to implement any package.
