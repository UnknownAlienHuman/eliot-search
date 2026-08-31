# W2 package checkpoints — `search-revision-store`

**Write scope:** `crates/search-source/search-revision-store/**`  
**Authority:** none; accepted G0/W1, issued ticket, active lease and acknowledged context remain required.

Read only the materialized package context, exact accepted dependency handoffs, assignment,
`FUNCTIONS.md`, this packet and the common rules in `../W2_MILESTONE_PACKETS.md`.

## V0 — Revision identity and residency

Implement exact source/revision identity, residency domains, configuration validation, immutable keys and open/read guards.

## V1 — Immutable admission and publication

Implement bounded CAS admission, temporary/write/fsync/rename/control ordering, idempotency and conflict semantics.

## V2 — Readback, leases and recovery

Implement exact digest/length readback, bounded leases, unknown-outcome recovery, protected revisions and explicit unavailability.

## V3 — Crash and lifecycle-boundary closure

Close every admission/publication crash point, residency mismatch, duplicate/replay, disclosure, line budget and submission evidence without implementing W7 purge.

## Exit rule

Each checkpoint records failing-first tests, exact commands/raw outcomes, typed gaps, package-only diff,
dependency/API digests and line count. V3 creates only a package submission candidate.
