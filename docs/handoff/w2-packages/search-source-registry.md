# W2 package checkpoints — `search-source-registry`

**Write scope:** `crates/search-source/search-source-registry/**`  
**Authority:** none; accepted G0/W1, issued ticket, active lease and acknowledged context remain required.

Read only the materialized package context, exact accepted dependency handoffs, assignment,
`FUNCTIONS.md`, this packet and the common rules in `../W2_MILESTONE_PACKETS.md`.

## RG0 — Snapshot and admitted roots

Implement bounded registry snapshot validation/digest/redaction, root registration, policy fencing and unbind obligations.

## RG1 — Sources, memberships and portfolios

Implement exact admission-receipt verification, source registration, membership transitions and explicit reference portfolios.

## RG2 — Views and namespace cutover

Implement coherent source/workspace views, stale-view verification and old-owner-fenced-before-new-owner activation.

## RG3 — Recovery and non-disclosure closure

Close mutation/cutover crash recovery, batch accounting, foreign-membership non-disclosure, no searchable/content state, line budget and submission evidence.

## Exit rule

Each checkpoint records failing-first tests, exact commands/raw outcomes, typed gaps, package-only diff,
dependency/API digests and line count. RG3 creates only a package submission candidate.
