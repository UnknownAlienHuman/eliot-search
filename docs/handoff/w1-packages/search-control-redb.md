# W1 package checkpoints — `search-control-redb`

**Write scope:** `crates/search-control-redb/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, package assignment,
`FUNCTIONS.md`, this packet and the common rules in `../W1_MILESTONE_PACKETS.md`.

## J0 — Journal identity and open

Implement exact journal identity, inspect/open/create guards, owner binding, schema/table verification, config validation, and quarantine.

## J1 — Migration

Implement explicit migration plan, transactional safe points, exact verification, cancellation/unknown outcome, and unsupported-version quarantine.

## J2 — Transactions and snapshots

Implement side-effect-free read snapshots, guarded idempotent transactions, recovery readback, control snapshot rebuild/publication, and pruning.

## J3 — Power-loss qualification

Close create/reopen/migration/commit/publication crash matrices, zero-hot-read-write proof, content/vendor guards, line budget, and handoff evidence.

## Exit rule

At every checkpoint record failing-first tests, exact commands/raw outcomes, typed gaps, package-only
diff, dependency/API digests and line count. The fourth checkpoint creates only a submission candidate;
independent review and integration-owned handoff remain separate.
