# Ticket-issuance planner qualification

This directory qualifies the **read-only advisory planner**. It does not qualify a context materializer,
ticket issuer, lease issuer, package implementation, gate or wave.

## Commands

```powershell
python -m py_compile tools/plan-ticket-issuance.py qualification/ticket-issuance/test_plan_ticket_issuance.py
python qualification/ticket-issuance/test_plan_ticket_issuance.py
python tools/plan-ticket-issuance.py --package search-contracts --output ticket-issuance-plan.json
```

The repository integration case must produce:

```text
decision = BLOCKED_MISSING_SELECTION
mutations = []
authorizes_ticket_issuance = false
creates_writer_lease = false
authorizes_implementation = false
publishes_package_handoff = false
advances_launch_state = false
```

That result is expected: the repository has non-claimable drafts but no selected immutable base commit,
writer or independent reviewer.

## Corpus

`cases.toml` defines 18 required cases covering:

- deterministic canonical output and non-circular plan digest;
- complete, partial and absent issuance selections;
- actor grammar and reviewer independence;
- immutable base-commit validation;
- missing/symlink/traversal context sources;
- unsupported selectors;
- claimable or prematurely resolved drafts;
- conditional handoff prerequisites and unexpected handoffs;
- protected issued-record roots and output paths;
- manual-only/read-only workflow policy;
- zero control-plane mutations and false authority flags in every result.

## Evidence boundary

A green test run proves only that the planner fails closed and cannot emit authority. It is not:

- context materialization evidence;
- an issued assignment ticket;
- a writer lease or acknowledgement;
- a package submission/review/handoff;
- G0 evidence;
- a W0 receipt;
- permission to implement `search-contracts`.
