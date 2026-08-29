# Reconfiguration contract 1.1 — Composite obligations

**Status:** accepted implementation correction.  
**Supersedes:** only the scalar `dominant_action` / total-order wording in sections 4–5 of
[`CONFIGURATION_1.0.md`](CONFIGURATION_1.0.md). Field definitions, ownership, security floors and
execution order remain valid.

## Problem

A configuration change can require multiple independent actions. For example:

```text
lexical profile change
  → NEW_COLLECTION_GENERATION
  + REBUILD_PROJECTION
  + route cutover/readback receipts

restrictive source policy change
  → SECURITY_BARRIER
  + dependent invalidation
  + explicit reconcile/rebuild work
```

Selecting one “most restrictive” enum can erase another required obligation. That is unsafe.

## Correct model

```yaml
SectionReloadDecision:
  section_name: ConfigSectionName
  changed_key_paths: bounded_set<ConfigKeyPath>
  required_actions: bounded_set<ReconfigurationAction>
  restrictive: bool
  affected_capabilities: bounded_set<PackageName>
  required_receipt_kinds: bounded_set<ReceiptKind>

ReconfigurationPlan:
  old_fingerprint: ConfigFingerprint
  candidate_fingerprint: ConfigFingerprint
  required_actions: bounded_set<ReconfigurationAction>
  ordered_steps: bounded_list<ReconfigurationStep>
  affected_capabilities: bounded_set<PackageName>
  required_receipt_kinds: bounded_set<ReceiptKind>
  blocking_reasons: bounded_set<ConfigReasonCode>
```

`ReconfigurationAction` remains the closed set:

```text
APPLY_LIVE
SECURITY_BARRIER
RESTART_DEPENDENCY
DRAIN_AND_RESTART
NEW_COLLECTION_GENERATION
REBUILD_PROJECTION
GATE_REQUIRED
REJECT
```

`NOOP` is represented by an empty action set.

## Composition rules

- `REJECT` blocks the candidate snapshot and no step executes.
- `GATE_REQUIRED` blocks activation until every named gate/ADR/artifact/feature receipt exists.
- `SECURITY_BARRIER` may coexist with restart, rebuild or generation actions.
- `NEW_COLLECTION_GENERATION` does not imply that projection rebuild, final publication barrier, route
  switch or old-route drain receipts may be omitted.
- `DRAIN_AND_RESTART` may subsume process timing but does not erase a security or migration receipt.
- A section minimum is an action floor. A field/owner may add actions but never remove the floor.
- The planner performs a deterministic topological sort over action prerequisites; it does not compare
  unrelated actions by one severity number.
- The old effective snapshot remains authoritative until every required step and receipt succeeds.
- Partial execution is either compensated or leaves the affected capability explicitly
  fail-closed/quarantined; it never publishes a mixed snapshot.

## Required fixtures

```text
security_and_restart_obligations_both_preserved
generation_and_projection_rebuild_both_preserved
gate_required_blocks_all_execution
reject_has_no_side_effect
deterministic_topological_step_order
failed_step_never_publishes_candidate_fingerprint
rollback_or_fail_closed_receipt_required
```
