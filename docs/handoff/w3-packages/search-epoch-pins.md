# W3 package checkpoints — `search-epoch-pins`

**Write scope:** `crates/search-index-qdrant/search-epoch-pins/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, applicable probe inputs, package assignment, `FUNCTIONS.md`, this packet and common rules in `../W3_MILESTONE_PACKETS.md`.

## EP0 — Pin identities and bounds

Implement route/epoch/owner/request pin identities, finite TTL/quota/owner bounds, canonical validation and content-minimized receipts.

## EP1 — Acquire, verify and release

Implement idempotent acquire/release, generation/route verification and exact active-pin observations. A stale or foreign pin never protects another route.

## EP2 — Cleanup and recovery

Implement cancellation, disconnect, request completion and daemon-restart cleanup decisions plus unknown release outcomes without silent expiry authority.

## EP3 — Conformance and handoff candidate

Close concurrency, stale-generation, leak/cleanup, reclaim-blocking, bounded-resource and property fixtures, line budget and package submission evidence.

## Exit rule

Each checkpoint records failing-first tests, exact commands/raw outcomes, unavailable checks, package-only diff, dependency/profile digests and line count. EP3 creates only a submission candidate; independent review and integration-owned handoff remain separate.
