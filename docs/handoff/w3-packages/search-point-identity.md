# W3 package checkpoints — `search-point-identity`

**Write scope:** `crates/search-index-qdrant/search-point-identity/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, collection/probe inputs, package assignment, `FUNCTIONS.md`, this packet and common rules in `../W3_MILESTONE_PACKETS.md`.

## PI0 — Canonical key inputs

Implement validation and canonicalization of exact point-key inputs, projection membership, source/revision/representation/unit/profile identities and domain separation. Policy/display text cannot enter identity.

## PI1 — Point ID derivation

Implement deterministic full-digest and UUID derivation, canonical bytes/goldens, finite batch behavior and explicit unsupported-version outcomes.

## PI2 — Collision refusal

Implement full-digest readback comparison, same-ID/different-input refusal and non-overwrite receipts. No store mutation or silent regeneration under another identity.

## PI3 — Conformance and handoff candidate

Close canonical vectors, collision/adversarial/property tests, schema/public-type/vendor guards, bounded resource behavior, line budget and package submission evidence.

## Exit rule

Each checkpoint records failing-first tests, exact commands/raw outcomes, unavailable checks, package-only diff, dependency/schema digests and line count. PI3 creates only a submission candidate; independent review and integration-owned handoff remain separate.
