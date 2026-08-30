# Ticket-issuance planner index

## Authority and machine files

- `TICKET_ISSUANCE_PLANNER.md` — read-only operation contract and failure surface.
- `TICKET_ISSUANCE_PLANNER_DIGEST_RULE.md` — non-circular advisory plan digest.
- `../../swarm/ticket-issuance-planner.toml` — machine component registry.
- `../../swarm/ticket-issuance-plan-schema.toml` — closed output shape and non-authority invariants.
- `../../swarm/ticket-issuance-plan-digest-v1.toml` — digest profile.

## Executable tooling

- `../../tools/plan-ticket-issuance.py` — dependency-free read-only planner.
- `../../tools/plan-ticket-issuance.ps1` — Windows wrapper.
- `../../tools/validate-ticket-issuance-plan.ps1` — structural/non-authority validator.

## Qualification

- `../../qualification/ticket-issuance/cases.toml` — 18-case closed corpus.
- `../../qualification/ticket-issuance/test_plan_ticket_issuance.py` — substantive tests.
- `../../qualification/ticket-issuance/run_planner_tests.py` — import-stable canonical runner.
- `../../.github/workflows/ticket-issuance-plan-qualified.yml` — manual Windows qualification.

## Current result

With no selected immutable base commit and no distinct writer/reviewer identities, the expected actual
repository decision is:

```text
BLOCKED_MISSING_SELECTION
```

The result always retains:

```text
mutations = []
authorizes_ticket_issuance = false
creates_writer_lease = false
authorizes_implementation = false
publishes_package_handoff = false
advances_launch_state = false
```

The planner is the final read-only preflight before a future integration-owned context-materialization
operation. It is not an implementation-agent ticket and does not unlock `search-contracts`.
