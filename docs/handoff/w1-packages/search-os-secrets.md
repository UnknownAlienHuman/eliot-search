# W1 package checkpoints — `search-os-secrets`

**Write scope:** `crates/search-runtime/search-os-secrets/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, package assignment,
`FUNCTIONS.md`, this packet and the common rules in `../W1_MILESTONE_PACKETS.md`.

## S0 — Binding and reference hygiene

Implement backend/profile validation, user/install/incarnation/purpose binding, opaque references, config validation, and public surface guards.

## S1 — Create and guarded use

Implement create, guarded borrowed use, bounded non-serializable leases, cancellation, zeroization claims, and content-free receipts.

## S2 — Rotation deletion recovery

Implement authoritative generation rotation, lease invalidation, logical deletion, exact unknown-outcome recovery, and quarantine.

## S3 — Side-channel qualification

Close argv/environment/log/error/metric/crash canaries, cross-binding denial, fault matrix, line budget, and submission evidence.

## Exit rule

At every checkpoint record failing-first tests, exact commands/raw outcomes, typed gaps, package-only
diff, dependency/API digests and line count. The fourth checkpoint creates only a submission candidate;
independent review and integration-owned handoff remain separate.
