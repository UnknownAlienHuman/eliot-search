# W1 package checkpoints — `eliot-searchd`

**Write scope:** `bins/eliot-searchd/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, package assignment,
`FUNCTIONS.md`, this packet and the common rules in `../W1_MILESTONE_PACKETS.md`.

## D0 — Composition profile

Implement W1-only profile validation, exact accepted handoff/API/config digests, private adapter graph, and future-wave exclusion.

## D1 — Startup shell

Implement owner acquisition, secret/control initialization, committed snapshot publication, endpoint startup, and crash recovery ordering.

## D2 — Readiness and requests

Implement coherent readiness/capability snapshot, connection/request admission, handler dispatch, cancellation/disconnect cleanup, and no hot control writes.

## D3 — Drain recovery handoff

Implement unresolved-operation recovery delegation, drain, reverse dependency shutdown, owner release, fault qualification, line budget, and submission evidence.

## Exit rule

At every checkpoint record failing-first tests, exact commands/raw outcomes, typed gaps, package-only
diff, dependency/API digests and line count. The fourth checkpoint creates only a submission candidate;
independent review and integration-owned handoff remain separate.
