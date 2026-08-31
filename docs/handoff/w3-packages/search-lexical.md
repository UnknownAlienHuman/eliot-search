# W3 package checkpoints — `search-lexical`

**Write scope:** `crates/search-lexical/**`  
**Authority:** none; an issued ticket, active lease and acknowledged context are still required.

Read only the materialized package context, exact accepted dependency handoffs, applicable W3 qualification inputs, package assignment, `FUNCTIONS.md`, this packet and common rules in `../W3_MILESTONE_PACKETS.md`.

## LX0 — Profile and tokenization

Freeze exact lexical/profile identity and implement canonical bounded tokenization, normalization, term/field accounting and deterministic profile validation. No corpus access or Qdrant client belongs here.

## LX1 — Statistics and filtered IDF

Implement deterministic document-frequency/statistics interfaces and the exact filtered-IDF semantics over supplied eligible populations. Inaccessible/denied populations cannot influence results.

## LX2 — Scores, batches and failure surfaces

Implement deterministic sparse values, stable tie inputs, bounded batches, cancellation/deadline/resource outcomes and content-minimized errors without ranking-policy ownership.

## LX3 — Conformance and handoff candidate

Close profile/token/score goldens, filtered-IDF noninterference, malformed/oversize/cancellation fixtures, dependency/vendor guards, line budget and package submission evidence.

## Exit rule

Each checkpoint records failing-first tests, exact commands/raw outcomes, unavailable checks, package-only diff, dependency/profile digests and line count. LX3 creates only a submission candidate; independent review and integration-owned handoff remain separate.
