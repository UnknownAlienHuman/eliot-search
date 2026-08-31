# W3 package checkpoints — `search-publication`

**Write scope:** `crates/search-index-qdrant/search-publication/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, accepted collection schema/failpoint inputs, package assignment, `FUNCTIONS.md`, this packet and common rules in `../W3_MILESTONE_PACKETS.md`.

## PUB0 — Publication intent and guards

Implement exact publication command/operation identity, expected route/epoch/profile/manifest guards and staged/uncommitted state. No visibility change occurs here.

## PUB1 — Point publication and readback

Implement bounded exact point writes, collision refusal, acknowledgements and full readback against planned identities. Unknown external outcomes remain explicit.

## PUB2 — Control commit and recovery

Implement serialized visibility/control commit only after readback, route/epoch transition receipts and recovery for every failpoint. Uncommitted epochs are never visible or reused.

## PUB3 — Conformance and handoff candidate

Close the publication crash matrix, collision/nonoverwrite, committed-but-unpublished recovery, no-broad-mutation guards, line budget and package submission evidence.

## Exit rule

Each checkpoint records failing-first tests, exact commands/raw outcomes, unavailable checks, package-only diff, dependency/schema/profile digests and line count. PUB3 creates only a submission candidate; independent review and integration-owned handoff remain separate.
