# W3 package checkpoints — `search-projection-planner`

**Write scope:** `crates/search-index-qdrant/search-projection-planner/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, accepted collection schema, package assignment, `FUNCTIONS.md`, this packet and common rules in `../W3_MILESTONE_PACKETS.md`.

## PP0 — Projection inputs and schema

Implement exact unit/representation/profile/membership/point-identity validation and accepted collection-schema binding. One projected point has one membership.

## PP1 — Deterministic plans and manifests

Implement canonical collection/projection plans, exact point manifests, payload/index requirements and deterministic diffs without store mutation.

## PP2 — Reprojection and retirement decisions

Implement changed-profile/revision/membership plan classification, created/retained/retired exact sets, bounded cancellation and explicit gaps.

## PP3 — Conformance and handoff candidate

Close plan/manifest goldens, missing-field/profile/schema mismatches, membership nonarray guard, bounds, line budget and package submission evidence.

## Exit rule

Each checkpoint records failing-first tests, exact commands/raw outcomes, unavailable checks, package-only diff, dependency/schema digests and line count. PP3 creates only a submission candidate; independent review and integration-owned handoff remain separate.
