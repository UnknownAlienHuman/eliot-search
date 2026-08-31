# W1 package checkpoints — `search-provider-protocol`

**Write scope:** `crates/search-provider-protocol/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, package assignment,
`FUNCTIONS.md`, this packet and the common rules in `../W1_MILESTONE_PACKETS.md`.

## P0 — Framing and limits

Implement exact frame/version schema, bounded decoding/encoding, sequence identities, finite connections/in-flight/bytes, and config validation.

## P1 — Pairing and binding

Implement local pairing, mutual authentication, binding/session lifecycle, replay denial, secret-use ports, and no authority grant.

## P2 — Request lifecycle

Implement request/progress/terminal state, backpressure, cancellation, deadline, exactly-one terminal result, and typed partial/degraded outcomes.

## P3 — Stress and handoff closure

Close saturation/disconnect/cleanup/replay/compatibility/fuzz fixtures, content disclosure, line budget, and package submission evidence.

## Exit rule

At every checkpoint record failing-first tests, exact commands/raw outcomes, typed gaps, package-only
diff, dependency/API digests and line count. The fourth checkpoint creates only a submission candidate;
independent review and integration-owned handoff remain separate.
