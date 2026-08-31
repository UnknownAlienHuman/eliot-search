# W1 package checkpoints — `search-runtime-owner`

**Write scope:** `crates/search-runtime/search-runtime-owner/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, package assignment,
`FUNCTIONS.md`, this packet and the common rules in `../W1_MILESTONE_PACKETS.md`.

## R0 — Root and owner identity

Implement local data-root canonicalization, process/executable/installation identity, owner epoch primitives, configuration validation, and failure tables.

## R1 — Acquire and verify

Implement observation, exact acquire idempotency, concurrent-owner denial, guard verification, cancellation, deadline, and unknown-outcome readback.

## R2 — Recovery and release

Implement abandoned-owner classification, safe recovery, drain token, release preconditions, clean release, and quarantine semantics.

## R3 — Fault and handoff closure

Close PID reuse, executable replacement, crash-boundary, replay/conflict, disclosure, line-budget, and package submission evidence.

## Exit rule

At every checkpoint record failing-first tests, exact commands/raw outcomes, typed gaps, package-only
diff, dependency/API digests and line count. The fourth checkpoint creates only a submission candidate;
independent review and integration-owned handoff remain separate.
